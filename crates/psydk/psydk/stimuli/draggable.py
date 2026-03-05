from psydk.visual.color import rgb
from psydk.visual.geometry import px
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle

class DraggableStimulus(BaseStimulus):
    def __init__(self, ctx, stimulus):
        """Wrapper stimulus that adds dragging behavior to any other stimulus."""

        self.stimulus = stimulus
        self.dragging = False
        self.drag_offset_x = 0
        self.drag_offset_y = 0

    def draw(self, scene, window_state):
        self.stimulus.draw(scene, window_state)

    def dispatch_event(self, event, window_state):
        if event.kind == "cursor_moved" and self.dragging:
            self.stimulus.set_position(px(event.position[0]) - self.drag_offset_x, px(event.position[1]) - self.drag_offset_y)
        elif event.kind == "mouse_button_press":
            if self.stimulus.contains_point(event.position, window_state):
                self.dragging = True
                self.drag_offset_x, self.drag_offset_y = self.stimulus.get_position()
                self .drag_offset_x = px(event.position[0]) - self.drag_offset_x
                self.drag_offset_y = px(event.position[1]) - self.drag_offset_y
        elif event.kind == "mouse_button_release":
            self.dragging = False

        return False
