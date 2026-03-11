from psydk.visual.color import rgb
from psydk.visual.geometry import px
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle

class DraggableStimulus(BaseStimulus):
    """Stimulus class that wraps another stimulus and allows it to be dragged with the mouse/finger.

    Parameters
    ----------
    ctx : object
        Experiment context provided by the psydk framework.
    stimulus : BaseStimulus
        The stimulus to be made draggable. This can be any stimulus that implements the `contains_point` method to detect if a point is within its bounds, and `get_position` and `set_position` methods to manage its position.
    """

    def __init__(self, ctx, stimulus):
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
