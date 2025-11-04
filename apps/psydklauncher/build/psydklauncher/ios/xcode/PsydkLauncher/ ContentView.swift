import SharedTypes
import Serde
import SwiftUI

extension SharedTypes.Experiment: Identifiable {}
extension SharedTypes.Subject: Identifiable {}
extension SharedTypes.Session: Identifiable {}
extension SharedTypes.Task: Identifiable {}




struct ContentView: View {
    @ObservedObject var core: Core
    
    init(core: Core) {
        self.core = core
    }
    

    
    @State private var selection: UInt128?
    

    
    @State private var isPythonRunning = false
    
    
    @State private var selected_experiment: SharedTypes.Experiment?
    @State private var selected_subject: SharedTypes.Subject?
    @State private var selected_session: SharedTypes.Session?
    @State private var selected_task: SharedTypes.Task?
    @State private var listSelection: Serde.UInt128?
    
    private func refresh_selected() {
        guard
            let currentExperiment = selected_experiment,
            let refreshedExperiment = core.view.experiments.first(where: { $0.id == currentExperiment.id })
        else {
            selected_experiment = nil
            selected_subject = nil
            selected_session = nil
            listSelection = nil
            return
        }

        selected_experiment = refreshedExperiment

        guard
            let currentSubject = selected_subject,
            let refreshedSubject = refreshedExperiment.subjects.first(where: { $0.id == currentSubject.id })
        else {
            selected_subject = nil
            selected_session = nil
            listSelection = nil
            return
        }

        selected_subject = refreshedSubject

        guard
            let currentSession = selected_session,
            let refreshedSession = refreshedSubject.sessions.first(where: { $0.id == currentSession.id })
        else {
            selected_session = nil
            listSelection = nil
            return
        }

        selected_session = refreshedSession
        listSelection = refreshedSession.id
    }
    

    
    
    
    private func binding(for subject: Subject) -> Binding<Bool> {
        Binding(
            get: {
                if let value = selected_subject {
                    value.id == subject.id
                } else {
                    false
                }
                
            },
            set: { $0 }
        )
    }
    
    @State private var newSessionName = ""
    @State private var newSubjectName = ""
    @State private var sessionDialogOpen = false
    @State private var subjectDialogOpen = false
    
    
    var body: some View {
        if isPythonRunning {
            VStack {
                ProgressView()
                Text("Running...")
            }
        }
        else if core.view.experiments.isEmpty {
            VStack {
                Text("Looks like there are no experiments available on this device.").font(.headline)
                Text("Use the the Files app or Finder to copy an experiment folder into this app's directory.")
                Button("Refresh experiments") {
                    core.update(.loadExperiments([FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!.path()]))
                }
                
            }
        }
        else {
            VStack {
                NavigationSplitView(columnVisibility:.constant(.all)) {
                    List(core.view.experiments, id: \.self, selection: $selected_experiment) { experiment in
                            VStack(alignment: .leading) {
                                HStack {
                                    Image(systemName: "questionmark.app")
                                    Text(experiment.name).font(.headline)
                                }
                                Text(experiment.description).foregroundColor(.secondary)
                                
                            }
                        }
                        .navigationTitle("Experiments")
                    
                    Button("Refresh experiments") {
                        core.update(.loadExperiments([FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!.path()]))
                    }
                    .buttonStyle(.bordered)
                } content: {
                    if let experiment = selected_experiment {
                        List {
                            Button{
                                subjectDialogOpen = true
                            } label: {
                                HStack {
                                    
                                    Image(systemName: "plus")
                                    Text("New subject")
                                    
                                }
                            }
                            .foregroundColor(.accentColor)
                            .padding(2)
                            .alert(
                                "Create a new subject for \(experiment.name)",
                                isPresented: $subjectDialogOpen,
                            ) {
                                TextField("New subject name", text: $newSubjectName)
                                    .textInputAutocapitalization(.never)
                                    .autocorrectionDisabled(true)

                                Button("Create") {
                                    core.update(.addNewSubject(experiment, newSubjectName))
                                    refresh_selected()
                                    newSubjectName = ""
                                }
                                Button("Cancel", role: .cancel) {
                                    newSubjectName = ""
                                }
                                
                            } message: {
                                Text("Enter subject name without sub-prefix")
                            }
                            ForEach(experiment.subjects) { subject in
                                let isExpanded = binding(for: subject)
                                
                                Section(isExpanded: isExpanded) {
                                    
                                    ForEach(subject.sessions) { session in
                                        
                                        HStack {
                                            Image(systemName: "folder")
                                            Text(session.name)
                                                .foregroundColor(listSelection == session.id ? Color.white : Color.black)
                                            Spacer()}
                                        .contentShape(Rectangle())
                                        .onTapGesture {
                                            listSelection = session.id
                                            selected_session = session
                                        }
                                        
                                        .listRowBackground(listSelection == session.id ? Color.accentColor.opacity(1.0) : Color.white)
                                        .padding(5)
                                        
                                    }
                                    Button{
                                        sessionDialogOpen = true
                                    } label: {
                                        HStack {
                                            
                                            Image(systemName: "plus.rectangle.on.folder")
                                            Text("New session for \(subject.name)")
                                            
                                        }
                                    }
                                    .foregroundColor(.accentColor)
                                    .padding(2)
                                    .alert(
                                        "Create a new session for \(subject.name)",
                                        isPresented: $sessionDialogOpen,
                                    ) {
                                        TextField("New session name", text: $newSessionName)
                                            .textInputAutocapitalization(.never)
                                            .autocorrectionDisabled(true)

                                        Button("Create") {
                                            core.update(.addNewSession(subject, newSessionName))
                                            refresh_selected()
                                            newSessionName = ""
                                        }
                                        Button("Cancel", role: .cancel) {
                                            newSessionName = ""
                                        }
                                        
                                    } message: {
                                        Text("Enter session name without ses-prefix")
                                    }
                                    
                                } header: {
                                    Button {
                                        selected_subject = subject
                                    } label: {
                                        HStack {
                                            Image(systemName: "person")
                                            
                                            Text(subject.name)
                                                .font(.headline)
                                            Spacer()
                                            Image(systemName: "chevron.right")
                                                .rotationEffect(.degrees(isExpanded.wrappedValue ? 90 : 0))
                                                .foregroundStyle(.secondary)
                                        }
                                        
                                    }
                                    .buttonStyle(.borderless)
                                    .controlSize(.extraLarge)
                                    .padding(8)
                                }
                                .textCase(nil)
                                .contentMargins(0)
                                
                                
                                
                            }
                            
                            
                        }
                        .listStyle(.inset)
                        .listSectionSpacing(0)
                        .navigationTitle("Sessions")
                    }
                    } detail: {
                        if let experiment = selected_experiment {
                   
                            List {
                                VStack {
                                    Picker("Tasks", selection:  Binding(
                                        get: {
                                            selected_task ?? experiment.default_task
                                        },
                                        set:
                                            {
                                                selected_task = $0
                                            }
                                    )) {
                                        ForEach(experiment.tasks) { task in
                                            Text(task.name)
                                                .tag(task)
                                        }
                                    }
                                    .pickerStyle(.segmented)
                                    Button("Start new run", systemImage: "play", action: {
                                        isPythonRunning = true
                                        let env: [String: String] = [
                                            "PSYDK_SUBJECT": selected_subject?.name ?? "",
                                            "PSYDK_SESSION": selected_session?.name ?? "",
                                            "PSYDK_TASK": selected_task?.name ?? experiment.default_task.name,
                                            "PSYDK_DATA_ROOT": experiment.directory + "/data"]
    
                                        let module = URL(filePath: experiment.directory, directoryHint: .isDirectory)
                                        let module_parent_path = module.deletingLastPathComponent().path
                                        let module_name = module.lastPathComponent
                                        
                                        print("Running \(module_name) in \(module_parent_path) with environment \(env)")
                                        
                                        DispatchQueue.global(qos: .userInitiated).async { StartPython(module_parent_path, module_name, env)   }
                                        
                                    }).buttonStyle(.borderedProminent)
                                        .tint(.green)
                                        .disabled(isPythonRunning || selected_experiment == nil || selected_subject == nil || selected_session == nil)
                                    
                                    
                                    
                                    Text("Experiment: \(selected_experiment?.name ?? "None")")
                                    Text("Subject: \(selected_subject?.name ?? "None")")
                                    Text("Session: \(selected_session?.name ?? "None")")
                                    //Text("Task: \(selected_task.name ?? "None")")
                                }
                            }
                        
                    }
            
                }
                
            }
        }
    
    }
    
        
}


struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView(core: Core())
    }
}
