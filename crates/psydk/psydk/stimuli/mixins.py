class ClickableMixin:
    """Mixin that adds click handling capabilities to a stimulus. It manages the click status (normal, hovered, pressed) and allows registering custom click event handlers."""

    def __init__(self):
        self.click_status = "normal"  # Can be "normal", "hovered", or "pressed"
        self.click_handlers = []
        self.was_clicked = False  # Flag to track if the stimulus was clicked

    def handle_click_event(self, event, window_state):
        self.was_clicked = True  # Set the clicked flag to True when a click event is handled
        for handler in self.click_handlers:
            handler(event)

    def add_click_handler(self, handler):
        self.click_handlers.append(handler)

    def clicked(self):
        """Returns True if the stimulus was clicked since the last check, and resets the clicked status."""
        was_clicked = self.was_clicked
        self.was_clicked = False  # Reset clicked status after checking
        return was_clicked

    def dispatch_event(self, event, window_state):
        if event.kind == "cursor_moved":
            if self.contains_point(event.position, window_state) and self.click_status != "pressed":
                self.click_status = "hovered"
            elif self.click_status == "hovered":
                self.click_status = "normal"
        elif event.kind == "mouse_button_press":
            if self.contains_point(event.position, window_state):
                self.click_status = "pressed"
        elif event.kind == "mouse_button_release" and self.click_status == "pressed":
            if self.contains_point(event.position, window_state):
                self.handle_click_event(event, window_state)  # Call the custom event handler for this button
                self.click_status = "hovered"
            else:
                self.click_status = "normal"

        return False  # Return False to allow event to propagate to other stimuli
