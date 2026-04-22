import SwiftUI

/// Modal sheet for the "File → Importa da URL…" command. Accepts an HTTP(S)
/// URL and hands it to the owning app, which calls `host.importUrl(_:)`.
/// The URL ingest is the one runtime code path that may touch the network —
/// user-initiated and explicit, consistent with the offline-only invariant.
public struct UrlImportSheet: View {
    @Binding var isPresented: Bool
    var onImport: (String) -> Void

    @State private var urlText: String = ""
    @State private var error: String? = nil

    public init(isPresented: Binding<Bool>, onImport: @escaping (String) -> Void) {
        self._isPresented = isPresented
        self.onImport = onImport
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                Text("IMPORTA DA URL")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("Incolla un URL")
                    .font(.serif(22, weight: .medium))
                    .foregroundStyle(Tokens.text)
                Text("Marginalia scaricherà l'articolo, pulirà il contenuto e lo aggiungerà alla libreria.")
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(3)
            }

            TextField("https://…", text: $urlText)
                .textFieldStyle(.roundedBorder)
                .font(.mono(12))
                .onSubmit(submit)

            if let error {
                Text(error)
                    .font(.serif(12, italic: true))
                    .foregroundStyle(.red.opacity(0.8))
            }

            HStack {
                Spacer()
                Button("Annulla") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
                Button("Importa") { submit() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(urlText.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(28)
        .frame(width: 520)
        .background(Tokens.bg)
    }

    private func submit() {
        let trimmed = urlText.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else {
            error = "URL vuoto."
            return
        }
        guard URL(string: trimmed)?.scheme?.hasPrefix("http") == true else {
            error = "URL deve iniziare con http:// o https://."
            return
        }
        onImport(trimmed)
        isPresented = false
    }
}
