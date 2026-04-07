"""OpenBrowser Playwright adapter.

Drop-in replacement for playwright.sync_api and playwright.async_api.
Uses OpenBrowser's CDP server under the hood.
"""

from open_playwright.sync_api import sync_playwright
from open_playwright.async_api import async_playwright

__version__ = "0.1.0"
__all__ = ["sync_playwright", "async_playwright"]
