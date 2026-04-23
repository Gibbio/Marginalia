// Menu bar + keyboard shortcuts for the shipped app. Mirrors the preview
// target's `MarginaliaCommands`; keep them in sync until the divergence makes
// a generic `MarginaliaCommands<Host>` in the `MarginaliaUI` library worth it.

import SwiftUI
import AppKit
import UniformTypeIdentifiers
import MarginaliaUI

struct LiveMarginaliaCommands: Commands {
    @ObservedObject var host: FFIHost
    @ObservedObject var uiState: LiveAppUIState

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Importa documento…") { importFile() }
                .keyboardShortcut("o", modifiers: [.command])
            Button("Importa da URL…") { uiState.showingUrlImport = true }
                .keyboardShortcut("u", modifiers: [.command])
            Divider()
            Button("Esporta note…") { exportNotes() }
                .keyboardShortcut("e", modifiers: [.command, .shift])
                .disabled(host.currentSession == nil || host.notes.isEmpty)
            Divider()
            Button("Esporta backup…") { exportBackup() }
            Button("Importa backup…") { importBackup() }
            Divider()
            Button("Chiudi sessione") { Task { try? await host.stop() } }
                .keyboardShortcut("w", modifiers: [.command])
                .disabled(host.currentSession == nil)
        }

        CommandMenu("Lettura") {
            Button(host.currentSession?.playbackState == .playing ? "Pausa" : "Riprendi") {
                Task {
                    if host.currentSession?.playbackState == .playing {
                        try? await host.pause()
                    } else {
                        try? await host.resume()
                    }
                }
            }
            .keyboardShortcut(.space, modifiers: [])
            .disabled(host.currentSession == nil)

            Divider()
            Button("Chunk successivo") { Task { try? await host.next() } }
                .keyboardShortcut(.rightArrow, modifiers: [.command])
                .disabled(host.currentSession == nil)
            Button("Chunk precedente") { Task { try? await host.back() } }
                .keyboardShortcut(.leftArrow, modifiers: [.command])
                .disabled(host.currentSession == nil)
            Button("Ripeti chunk") { Task { try? await host.repeatCurrent() } }
                .keyboardShortcut("r", modifiers: [.command])
                .disabled(host.currentSession == nil)

            Divider()
            Button("Capitolo successivo") { Task { try? await host.nextChapter() } }
                .keyboardShortcut(.rightArrow, modifiers: [.command, .shift])
                .disabled(host.currentSession == nil)
            Button("Capitolo precedente") { Task { try? await host.previousChapter() } }
                .keyboardShortcut(.leftArrow, modifiers: [.command, .shift])
                .disabled(host.currentSession == nil)

            Divider()
            Button("Nuova nota dettata") { host.startDictation() }
                .keyboardShortcut("n", modifiers: [.command])
                .disabled(host.currentSession == nil)
            Button("Elenco note") {
                NotificationCenter.default.post(name: .marginaliaShowNotes, object: nil)
            }
            .keyboardShortcut("n", modifiers: [.command, .option])
            .disabled(host.currentSession == nil)
            Button("Salva segnalibro") { Task { try? await host.bookmark() } }
                .keyboardShortcut("b", modifiers: [.command])
                .disabled(host.currentSession == nil)
            Button("Lista segnalibri") {
                NotificationCenter.default.post(name: .marginaliaShowBookmarks, object: nil)
            }
            .keyboardShortcut("b", modifiers: [.command, .option])
            .disabled(host.currentSession == nil)
            Button("Dove sono") { _ = host.announcePosition() }
                .keyboardShortcut("?", modifiers: [.command])
                .disabled(host.currentSession == nil)

            Divider()
            // Volume in 10% steps. Bound to the slider's [0,1] range —
            // future work can extend to rodio's amplification beyond 1.0
            // with a separate voice command ("più forte ancora").
            Button("Volume più alto") { host.volume = min(1.0, host.volume + 0.1) }
                .keyboardShortcut(.upArrow, modifiers: [.command])
            Button("Volume più basso") { host.volume = max(0.0, host.volume - 0.1) }
                .keyboardShortcut(.downArrow, modifiers: [.command])
        }

        CommandGroup(replacing: .appSettings) {
            Button("Impostazioni…") {
                NotificationCenter.default.post(name: .marginaliaOpenSettings, object: nil)
            }
            .keyboardShortcut(",", modifiers: [.command])
        }

        CommandGroup(after: .toolbar) {
            Button("Mostra / nascondi log") {
                NotificationCenter.default.post(name: .marginaliaToggleLog, object: nil)
            }
            .keyboardShortcut("l", modifiers: [.option])
        }

        CommandGroup(replacing: .help) {
            Button("Scorciatoie") {
                NotificationCenter.default.post(name: .marginaliaShowShortcuts, object: nil)
            }
            .keyboardShortcut("/", modifiers: [.command])
        }
    }

    private func importFile() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowedContentTypes = [.pdf, .epub, .plainText]
        if panel.runModal() == .OK, let url = panel.url {
            Task {
                _ = try? await host.importFile(url: url)
                await host.refreshLibrary()
            }
        }
    }

    private func exportBackup() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.zip]
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withFullDate]
        panel.nameFieldStringValue = "marginalia-\(iso.string(from: Date())).zip"
        panel.canCreateDirectories = true
        panel.title = "Esporta backup Marginalia"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        Task {
            do {
                try await host.exportBackup(path: url.path)
                await MainActor.run {
                    host.transientToast = ToastMessage(
                        text: "Backup salvato.", kind: .info
                    )
                }
            } catch {
                await MainActor.run {
                    host.pushMessage("Errore export backup: \(error.localizedDescription)")
                }
            }
        }
    }

    private func importBackup() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.zip]
        panel.allowsMultipleSelection = false
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.title = "Importa backup Marginalia"
        guard panel.runModal() == .OK, let url = panel.url else { return }

        // Confirmation — import overwrites marginalia.toml and the
        // sqlite db. Anything saved after the backup is lost unless the
        // user exported first.
        let alert = NSAlert()
        alert.messageText = "Importare questo backup?"
        alert.informativeText = "Le preferenze, la libreria e le note correnti verranno sostituite con quelle nel backup. Marginalia dovrà essere riavviata per caricare i dati ripristinati."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Importa e esci")
        alert.addButton(withTitle: "Annulla")
        guard alert.runModal() == .alertFirstButtonReturn else { return }

        Task {
            do {
                try await host.importBackup(path: url.path)
                await MainActor.run { NSApp.terminate(nil) }
            } catch {
                await MainActor.run {
                    host.pushMessage("Errore import backup: \(error.localizedDescription)")
                }
            }
        }
    }

    private func exportNotes() {
        let md = host.exportNotesMarkdown()
        guard !md.isEmpty else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.plainText]
        let safeTitle = (host.currentSession?.documentTitle ?? "note")
            .replacingOccurrences(of: "/", with: "-")
        panel.nameFieldStringValue = "\(safeTitle) — note.md"
        panel.canCreateDirectories = true
        if panel.runModal() == .OK, let url = panel.url {
            do {
                try md.write(to: url, atomically: true, encoding: .utf8)
            } catch {
                host.pushMessage("Errore export note: \(error.localizedDescription)")
            }
        }
    }
}
