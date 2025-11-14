import SwiftUI

@main
struct CounterAppApp: App {
    var core = Core();
    init() {
        
        let loadPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!.path()
        
        // make sure there is a README.md in there
        if !FileManager.default.fileExists(atPath: loadPath + "/README.md") {
            // create one!
            FileManager.default.createFile(atPath: loadPath + "/README.md", contents: Data("# Welcome to SwiftUICounter!\n\nThis is a demo of using SwiftUI to build a simple counter.\n\nTo get started, just open the app and click the button to increment the counter.\n\n".utf8))
        }

        core.update(.loadExperiments([loadPath]))
        
    }
    var body: some Scene {
        WindowGroup {
            ContentView(core: core)
        }
    }
}
