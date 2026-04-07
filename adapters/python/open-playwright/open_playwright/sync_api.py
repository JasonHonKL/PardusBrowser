"""Synchronous Playwright-compatible API backed by OpenBrowser."""

from __future__ import annotations

import os
import subprocess
import sys
import time
import atexit
from typing import Optional
from contextlib import contextmanager

import playwright.sync_api
from playwright.sync_api import sync_playwright as _native_sync_playwright


def _find_open_browser() -> str:
    for candidate in [
        os.environ.get("OPEN_BROWSER_PATH"),
        "open-browser",
    ]:
        if candidate is None:
            continue
        if os.path.isfile(candidate):
            return candidate
        try:
            result = subprocess.run(
                ["which", candidate], capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0:
                return result.stdout.strip()
        except (subprocess.TimeoutExpired, FileNotFoundError):
            continue
    raise FileNotFoundError(
        "open-browser not found. Install OpenBrowser or set OPEN_BROWSER_PATH."
    )


class OpenLauncher:
    def __init__(
        self,
        host: str = "127.0.0.1",
        port: Optional[int] = None,
        timeout: int = 10,
        headless: bool = True,
        binary_path: Optional[str] = None,
    ):
        self.host = host
        self.port = port
        self.timeout = timeout
        self.headless = headless
        self.binary_path = binary_path or _find_open_browser()
        self._process: Optional[subprocess.Popen] = None
        self._cdp_url: Optional[str] = None

    def start(self) -> str:
        if self.port is None:
            import socket
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.bind(("127.0.0.1", 0))
                self.port = s.getsockname()[1]

        cmd = [
            self.binary_path,
            "serve",
            "--host", self.host,
            "--port", str(self.port),
        ]

        self._process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        self._cdp_url = f"http://{self.host}:{self.port}"

        deadline = time.time() + self.timeout
        while time.time() < deadline:
            if self._process.poll() is not None:
                stderr = self._process.stderr.read().decode() if self._process.stderr else ""
                self._process = None
                raise RuntimeError(f"open-browser exited early: {stderr}")
            try:
                import urllib.request
                urllib.request.urlopen(f"{self._cdp_url}/json/version", timeout=1)
                return self._cdp_url
            except Exception:
                time.sleep(0.2)

        self.stop()
        raise TimeoutError(
            f"open-browser did not start within {self.timeout}s"
        )

    def stop(self):
        if self._process and self._process.poll() is None:
            self._process.terminate()
            try:
                self._process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._process.kill()
            self._process = None
        self._cdp_url = None


@contextmanager
def sync_playwright():
    """Synchronous Playwright context manager that uses OpenBrowser.

    Usage::

        with sync_playwright() as p:
            browser = p.chromium.launch()
            page = browser.new_page()
            page.goto("https://example.com")
            content = page.content()
            browser.close()
    """
    launcher = OpenLauncher()
    cdp_url = launcher.start()

    try:
        with _native_sync_playwright() as native_p:
            browser = native_p.chromium.connect_over_cdp(
                cdp_url,
                timeout=launcher.timeout * 1000,
            )

            class OpenPlaywrightContext:
                def __init__(self, native, _launcher):
                    self.chromium = _OpenBrowserType(native.chromium, _launcher)
                    self.firefox = native.firefox
                    self.webkit = native.webkit
                    self.request = native.request
                    self.selectors = native.selectors
                    self.devices = native.devices

            yield OpenPlaywrightContext(native_p, launcher)
    finally:
        launcher.stop()


class _OpenBrowserType:
    def __init__(self, native_type, launcher: OpenLauncher):
        self._native = native_type
        self._launcher = launcher

    def launch(self, **kwargs):
        cdp_url = self._launcher._cdp_url
        if not cdp_url:
            raise RuntimeError("OpenBrowser is not running")

        browser = self._native.connect_over_cdp(cdp_url)
        browser._open_launcher = self._launcher
        return browser

    def connect_over_cdp(self, endpoint_url, **kwargs):
        return self._native.connect_over_cdp(endpoint_url, **kwargs)
