from __future__ import annotations

import time
from dataclasses import dataclass, field
from enum import Enum

from notch.core.config import AppConfig


class NotchState(str, Enum):
    HIDDEN = "hidden"
    COLLAPSED = "collapsed"
    PEEK = "peek"
    EXPANDED = "expanded"


class ExpandedTab(Enum):
    OVERVIEW = ("overview", "Home")
    MEDIA = ("media", "Media")
    CONTROLS = ("controls", "Audio")
    TASKS = ("tasks", "Tasks")
    CALENDAR = ("calendar", "Cal")
    TIMER = ("timer", "Timer")
    APPS = ("apps", "Apps")
    RACK = ("rack", "Rack")

    @property
    def id(self) -> str:
        return self.value[0]

    @property
    def label(self) -> str:
        return self.value[1]


@dataclass
class SpringValue:
    value: float
    target: float = 0.0
    velocity: float = 0.0

    def __post_init__(self) -> None:
        self.target = self.value

    def set_target(self, target: float) -> None:
        self.target = target

    def is_moving(self) -> bool:
        return abs(self.value - self.target) > 0.25 or abs(self.velocity) > 0.5

    def step(self, dt: float) -> bool:
        dt = max(0.001, min(0.05, dt))
        stiffness = 340.0
        damping = 32.0
        accel = stiffness * (self.target - self.value) - damping * self.velocity
        self.velocity += accel * dt
        self.value += self.velocity * dt
        if not self.is_moving():
            self.value = self.target
            self.velocity = 0.0
            return False
        return True


@dataclass
class PeekBanner:
    title: str
    subtitle: str
    started_at: float
    duration_secs: float


@dataclass
class TickOutcome:
    needs_timer: bool
    needs_repaint: bool


@dataclass
class NotchStateMachine:
    config: AppConfig
    now: float | None = None
    state: NotchState = NotchState.COLLAPSED
    active_tab: ExpandedTab = ExpandedTab.OVERVIEW
    width: SpringValue = field(init=False)
    height: SpringValue = field(init=False)
    corner_radius: SpringValue = field(init=False)
    peek: PeekBanner | None = None
    fullscreen_hidden: bool = False
    manual_hidden: bool = False
    input_focused: bool = False
    _hover_active: bool = False
    _hover_since: float | None = None
    _leave_since: float | None = None
    _last_step: float = field(init=False)

    def __post_init__(self) -> None:
        self.width = SpringValue(self.config.collapsed_width)
        self.height = SpringValue(self.config.collapsed_height)
        self.corner_radius = SpringValue(self.config.collapsed_height * 0.5)
        self._last_step = self.now if self.now is not None else time.monotonic()

    def is_animating(self) -> bool:
        return self.width.is_moving() or self.height.is_moving() or self.corner_radius.is_moving()

    def _apply_targets(self, cfg: AppConfig) -> None:
        if self.state in (NotchState.HIDDEN, NotchState.COLLAPSED):
            self.width.set_target(cfg.collapsed_width)
            self.height.set_target(cfg.collapsed_height)
            self.corner_radius.set_target(cfg.collapsed_height * 0.5)
        elif self.state == NotchState.PEEK:
            self.width.set_target(cfg.peek_width)
            self.height.set_target(cfg.peek_height)
            self.corner_radius.set_target(20.0)
        elif self.state == NotchState.EXPANDED:
            self.width.set_target(cfg.expanded_width)
            self.height.set_target(cfg.expanded_height)
            self.corner_radius.set_target(22.0)

    def collapse(self, cfg: AppConfig) -> None:
        self.state = NotchState.HIDDEN if (self.fullscreen_hidden or self.manual_hidden) else NotchState.COLLAPSED
        self.peek = None
        self._hover_since = None
        self._leave_since = None
        self._apply_targets(cfg)

    def expand(self, tab: ExpandedTab, cfg: AppConfig) -> None:
        if self.fullscreen_hidden or self.manual_hidden:
            return
        self.state = NotchState.EXPANDED
        self.active_tab = tab
        self.peek = None
        self._hover_since = None
        self._leave_since = None
        self._apply_targets(cfg)

    def toggle_expand(self, cfg: AppConfig) -> None:
        if self.state == NotchState.EXPANDED:
            self.collapse(cfg)
        elif self.state in (NotchState.COLLAPSED, NotchState.PEEK):
            self.expand(self.active_tab, cfg)

    def trigger_peek(self, title: str, subtitle: str, cfg: AppConfig, now: float | None = None) -> None:
        if self.fullscreen_hidden or self.manual_hidden or self.state == NotchState.EXPANDED:
            return
        t = time.monotonic() if now is None else now
        self.state = NotchState.PEEK
        self.peek = PeekBanner(title=title, subtitle=subtitle, started_at=t, duration_secs=max(0.05, cfg.peek_duration_ms / 1000.0))
        self._apply_targets(cfg)

    def toggle_manual_hide(self, cfg: AppConfig) -> None:
        self.manual_hidden = not self.manual_hidden
        self.state = NotchState.HIDDEN if (self.manual_hidden or self.fullscreen_hidden) else NotchState.COLLAPSED
        self._apply_targets(cfg)

    def set_fullscreen_hidden(self, hidden: bool, cfg: AppConfig) -> None:
        if self.fullscreen_hidden == hidden:
            return
        self.fullscreen_hidden = hidden
        self.state = NotchState.HIDDEN if (hidden or self.manual_hidden) else NotchState.COLLAPSED
        self.peek = None
        self._apply_targets(cfg)

    def on_hover_change(self, hovering: bool, cfg: AppConfig, now: float | None = None) -> None:
        self._hover_active = hovering
        t = time.monotonic() if now is None else now
        if hovering:
            self._leave_since = None
            if self.state == NotchState.COLLAPSED and self._hover_since is None:
                self._hover_since = t
        else:
            self._hover_since = None
            if self.state == NotchState.EXPANDED and not self.input_focused and self._leave_since is None:
                self._leave_since = t

    def tick(self, cfg: AppConfig, now: float | None = None) -> TickOutcome:
        t = time.monotonic() if now is None else now
        dt = max(0.001, min(0.05, t - self._last_step))
        self._last_step = t
        changed = False

        if self.state == NotchState.PEEK and self.peek and (t - self.peek.started_at) >= self.peek.duration_secs:
            self.collapse(cfg)
            changed = True

        if self.state == NotchState.COLLAPSED and self._hover_active and self._hover_since is not None:
            if (t - self._hover_since) * 1000.0 >= cfg.hover_expand_delay_ms:
                self.expand(self.active_tab, cfg)
                changed = True
        elif self.state == NotchState.EXPANDED and not self._hover_active and not self.input_focused and self._leave_since is not None:
            if (t - self._leave_since) * 1000.0 >= cfg.hover_collapse_delay_ms:
                self.collapse(cfg)
                changed = True

        mw = self.width.step(dt)
        mh = self.height.step(dt)
        mr = self.corner_radius.step(dt)
        anim = mw or mh or mr
        pending = (self._hover_since is not None) or (self._leave_since is not None)
        return TickOutcome(needs_timer=anim or pending or self.state == NotchState.PEEK, needs_repaint=anim or changed or self.state == NotchState.PEEK)
