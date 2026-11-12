from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable

from notch.modules.file_rack import RackItem
from notch.ui.gdiplus import GdiPlusPainter
from notch.ui.theme import (
    ROSE, SKY, SURFACE, TEXT_DIM, TEXT_MUTED, TEXT_PRIMARY,
    blend_alpha, draw_card, draw_icon, draw_icon_button, draw_pill_button,
)
from notch.ui.widgets import Rect, UiInteractionState


@dataclass
class RackUiState:
    items: list[RackItem] = field(default_factory=list)


@dataclass
class RackCallbacks:
    on_add_file_dialog: Callable[[], None]
    on_open_item: Callable[[str], None]
    on_remove_item: Callable[[str], None]
    on_clear: Callable[[], None]


def draw_rack_panel(painter: GdiPlusPainter, interaction: UiInteractionState, state: RackUiState, s: float, x: float, y: float, w: float, cb: RackCallbacks) -> None:
    count = len(state.items)
    subtitle = f"{count} item{'s' if count != 1 else ''} · drag out to any app" if count else "Drag & drop files onto the notch (zero copies)"
    painter.draw_text(subtitle, x + 2.0 * s, y, w - 166.0 * s, 26.0 * s, TEXT_MUTED, size_px=10.5 * s)
    draw_pill_button(painter, interaction, "rack_add", x + w - 156.0 * s, y, 88.0 * s, 26.0 * s, "Add File", cb.on_add_file_dialog, active=True, accent=SKY, icon="plus", size_px=10.5 * s, radius=8.0 * s)
    draw_pill_button(painter, interaction, "rack_clear", x + w - 62.0 * s, y, 62.0 * s, 26.0 * s, "Clear", cb.on_clear, active=count > 0, accent=ROSE, size_px=10.5 * s, radius=8.0 * s)

    drop_y, drop_h = y + 34.0 * s, 194.0 * s
    draw_card(painter, x, drop_y, w, drop_h, radius=12.0 * s, accent=SKY if count else 0)
    if not state.items:
        painter.fill_circle(x + w * 0.5, drop_y + 72.0 * s, 22.0 * s, blend_alpha(SKY, 28))
        draw_icon(painter, "folder", x + w * 0.5, drop_y + 72.0 * s, 20.0 * s, SKY)
        painter.draw_text("Drop files or folders anywhere on the notch", x + 16.0 * s, drop_y + 106.0 * s, w - 32.0 * s, 22.0 * s, TEXT_PRIMARY, size_px=12.0 * s, bold=True, align=1)
        painter.draw_text("Stores references only — files stay untouched in place", x + 16.0 * s, drop_y + 128.0 * s, w - 32.0 * s, 18.0 * s, TEXT_DIM, size_px=10.0 * s, align=1)
        return
    for idx, item in enumerate(state.items[:4]):
        ry, row_h = drop_y + 10.0 * s + idx * 45.0 * s, 38.0 * s
        row_rect = Rect(x + 10.0 * s, ry, w - 120.0 * s, row_h)
        interaction.register_drag_source(f"rack_drag_{idx}", row_rect, [item.path])
        painter.fill_round_rect(row_rect.x, ry, w - 20.0 * s, row_h, 9.0 * s, SURFACE)
        draw_icon(painter, "grip", x + 21.0 * s, ry + row_h * 0.5, 13.0 * s, TEXT_DIM)
        ico_sz = 22.0 * s
        if not painter.draw_target_icon(item.path, x + 32.0 * s, ry + (row_h - ico_sz) * 0.5, ico_sz):
            draw_icon(painter, "folder", x + 43.0 * s, ry + row_h * 0.5, 14.0 * s, SKY)
        painter.draw_text(item.name, x + 62.0 * s, ry, w - 236.0 * s, row_h, TEXT_PRIMARY, size_px=11.5 * s, bold=True)
        painter.draw_text(item.size_label, x + w - 170.0 * s, ry, 58.0 * s, row_h, TEXT_MUTED, size_px=10.0 * s, mono=True, align=2)
        draw_pill_button(painter, interaction, f"rack_open_{idx}", x + w - 104.0 * s, ry + 7.0 * s, 56.0 * s, 24.0 * s, "Open", lambda p=item.path: cb.on_open_item(p), size_px=10.0 * s, radius=7.0 * s)
        draw_icon_button(painter, interaction, f"rack_rm_{idx}", x + w - 26.0 * s, ry + row_h * 0.5, 10.0 * s, "close", lambda p=item.path: cb.on_remove_item(p), fill=SURFACE, icon_color=ROSE)
