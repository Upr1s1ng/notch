from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable, Optional


@dataclass
class Rect:
    x: float
    y: float
    w: float
    h: float

    def contains(self, px: float, py: float) -> bool:
        return self.x <= px <= self.x + self.w and self.y <= py <= self.y + self.h


@dataclass
class HitTarget:
    id: str
    rect: Rect
    on_click: Callable[[], None]


@dataclass
class SliderTarget:
    id: str
    rect: Rect
    on_change: Callable[[float], None]


@dataclass
class TextInputState:
    text: str = ""
    cursor: int = 0
    focused: bool = False
    placeholder: str = ""

    def insert_text(self, s: str) -> None:
        self.text = self.text[: self.cursor] + s + self.text[self.cursor :]
        self.cursor += len(s)

    def backspace(self) -> None:
        if self.cursor > 0:
            self.text = self.text[: self.cursor - 1] + self.text[self.cursor :]
            self.cursor -= 1

    def delete(self) -> None:
        if self.cursor < len(self.text):
            self.text = self.text[: self.cursor] + self.text[self.cursor + 1 :]

    def clear(self) -> None:
        self.text = ""
        self.cursor = 0


@dataclass
class UiInteractionState:
    hit_targets: list[HitTarget] = field(default_factory=list)
    slider_targets: list[SliderTarget] = field(default_factory=list)
    input_rects: list[tuple[str, Rect]] = field(default_factory=list)
    drag_targets: list[tuple[str, Rect, list[str]]] = field(default_factory=list)
    hovered_id: Optional[str] = None
    pressed_id: Optional[str] = None
    active_slider_id: Optional[str] = None
    _pending_drag: Optional[tuple[float, float, list[str]]] = None
    pointer_x: float = -1.0
    pointer_y: float = -1.0

    def is_pressed(self, id: str) -> bool:
        return self.pressed_id == id

    def begin_frame(self) -> None:
        self.hit_targets.clear()
        self.slider_targets.clear()
        self.input_rects.clear()
        self.drag_targets.clear()

    def register_button(self, id: str, rect: Rect, on_click: Callable[[], None]) -> bool:
        self.hit_targets.append(HitTarget(id=id, rect=rect, on_click=on_click))
        return rect.contains(self.pointer_x, self.pointer_y)

    def register_slider(self, id: str, rect: Rect, on_change: Callable[[float], None]) -> bool:
        self.slider_targets.append(SliderTarget(id=id, rect=rect, on_change=on_change))
        return rect.contains(self.pointer_x, self.pointer_y)

    def register_input(self, id: str, rect: Rect) -> None:
        self.input_rects.append((id, rect))

    def register_drag_source(self, id: str, rect: Rect, paths: list[str]) -> None:
        self.drag_targets.append((id, rect, paths))

    def consume_drag_start(self, px: float, py: float) -> Optional[list[str]]:
        if not self._pending_drag:
            return None
        sx, sy, paths = self._pending_drag
        if abs(px - sx) >= 6.0 or abs(py - sy) >= 6.0:
            self._pending_drag = None
            return paths
        return None

    def update_hover(self, px: float, py: float) -> bool:
        self.pointer_x = px
        self.pointer_y = py
        if self.active_slider_id is not None:
            for s in self.slider_targets:
                if s.id == self.active_slider_id:
                    ratio = max(0.0, min(1.0, (px - s.rect.x) / max(1.0, s.rect.w)))
                    s.on_change(ratio)
                    return True
        new_hover = None
        for t in reversed(self.hit_targets):
            if t.rect.contains(px, py):
                new_hover = t.id
                break
        changed = new_hover != self.hovered_id
        self.hovered_id = new_hover
        return changed

    def handle_pointer_down(self, px: float, py: float) -> bool:
        self._pending_drag = None
        self.pressed_id = None
        for s in reversed(self.slider_targets):
            if s.rect.contains(px, py):
                self.active_slider_id = s.id
                s.on_change(max(0.0, min(1.0, (px - s.rect.x) / max(1.0, s.rect.w))))
                return True
        for _id, rect, paths in reversed(self.drag_targets):
            if paths and rect.contains(px, py):
                self._pending_drag = (px, py, paths)
                return False
        for t in reversed(self.hit_targets):
            if t.rect.contains(px, py):
                self.pressed_id = t.id
                return True
        return False

    def handle_click(self, px: float, py: float) -> Optional[str]:
        self._pending_drag = None
        self.pressed_id = None
        if self.active_slider_id is not None:
            self.active_slider_id = None
            return None
        for input_id, rect in self.input_rects:
            if rect.contains(px, py):
                return input_id
        for t in reversed(self.hit_targets):
            if t.rect.contains(px, py):
                t.on_click()
                return None
        return None
