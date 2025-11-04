//
//  Core.swift
//  Psydk Launcher
//
//  Created by Marc on 01/11/2025.
//  Copyright © 2025 Russell Keith-Magee. All rights reserved.
//


import Foundation
import SharedTypes

@MainActor
class Core: ObservableObject {
    @Published var view: ViewModel
    
    init() {
        self.view = try! .bincodeDeserialize(input: [UInt8](Psydk_Launcher.view()))
    }

    func update(_ event: Event) {
        let effects = [UInt8](processEvent(Data(try! event.bincodeSerialize())))

        let requests: [Request] = try! .bincodeDeserialize(input: effects)
        for request in requests {
            processEffect(request)
        }
    }

    func processEffect(_ request: Request) {
        switch request.effect {
        case .render:
            view = try! .bincodeDeserialize(input: [UInt8](Psydk_Launcher.view()))

        }
    }
}
