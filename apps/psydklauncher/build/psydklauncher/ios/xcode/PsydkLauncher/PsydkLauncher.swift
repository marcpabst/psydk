import SwiftUI

@main
struct CounterAppApp: App {
    var core = Core();
    init() {

        core.update(.loadExperiments([FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!.path()]))
        
    }
    var body: some Scene {
        WindowGroup {
            ContentView(core: core)
        }
    }
}
