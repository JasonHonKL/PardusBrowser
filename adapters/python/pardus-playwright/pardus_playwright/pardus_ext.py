"""Pardus extension namespace for Page objects.

Provides PardusBrowser-unique features via page.pardus.*
"""

from __future__ import annotations

import json
from typing import Any, Dict, List, Optional


class PardusPageExtension:
    """Extension methods for PardusBrowser's semantic features.

    Accessible via `page.pardus` when using the pardus-playwright adapter.
    """

    def __init__(self, cdp_session):
        self._cdp = cdp_session

    async def semantic_tree(self) -> Dict[str, Any]:
        """Get the semantic accessibility tree of the current page.

        Returns a tree with ARIA roles, element IDs, headings, landmarks,
        and interactive elements annotated with AI-friendly metadata.
        """
        result = await self._cdp.send("Pardus.semanticTree")
        return result.get("semanticTree", {})

    async def navigation_graph(self) -> Dict[str, Any]:
        """Get the navigation graph of the current page.

        Returns internal links, external links, and form descriptors.
        """
        result = await self._cdp.send("Pardus.getNavigationGraph")
        return result.get("navigationGraph", {})

    async def detect_actions(self) -> List[Dict[str, Any]]:
        """Detect all interactive elements on the current page.

        Returns a list of elements with their selectors, tags, actions,
        labels, hrefs, and disabled state.
        """
        result = await self._cdp.send("Pardus.detectActions")
        return result.get("actions", [])

    async def click_by_id(self, element_id: int) -> Dict[str, Any]:
        """Click an element by its semantic element ID (e.g., [#1]).

        This is the preferred way for AI agents to interact with elements,
        as semantic IDs are stable across page reloads.
        """
        result = await self._cdp.send("Pardus.interact", {
            "action": "click",
            "selector": f"[*data-eid='{element_id}']",
        })
        return result

    async def type_by_id(self, element_id: int, value: str) -> Dict[str, Any]:
        """Type text into a form field by its semantic element ID."""
        result = await self._cdp.send("Pardus.interact", {
            "action": "type",
            "selector": f"[*data-eid='{element_id}']",
            "value": value,
        })
        return result

    async def submit_form(self, form_selector: str, fields: Dict[str, str]) -> Dict[str, Any]:
        """Submit a form with accumulated field values."""
        result = await self._cdp.send("Pardus.interact", {
            "action": "submit",
            "selector": form_selector,
            "fields": [{"name": k, "value": v} for k, v in fields.items()],
        })
        return result
