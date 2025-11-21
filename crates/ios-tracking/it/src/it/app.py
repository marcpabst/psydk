"""
My first application
"""

import toga
from ios_tracking import FaceTracker
import asyncio


class it(toga.App):
    def startup(self):
        """Construct and show the Toga application.

        Usually, you would add your application to a main content box.
        We then create a main window (with a name matching the app), and
        show the main window.
        """
        main_box = toga.Box()

        self.main_window = toga.MainWindow(title=self.formal_name)
        self.main_window.content = main_box
        self.main_window.show()

        # add label
        self.label = toga.Label("No face detected", color="red")
        self.label.style.font_size = 48
        self.label.style.flex = 1
        main_box.add(self.label)

        self.ft = FaceTracker()

        # use self.loop to schedule periodic updates (loop is asyncio eventloop)
        self.add_background_task(self.periodic_update)

    async def periodic_update(self,_):
        while True:
            dist = self.ft.get_last_face_distance()
            if dist is not None:
                self.label.text = f"{dist:.3f} m"
                self.label.color = "green"
            else:
                self.label.text = "No face detected"
                self.label.color = "red"
            self.main_window.content.refresh()
            await asyncio.sleep(0.001)



def main():
    return it()
