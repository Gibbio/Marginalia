import Foundation
import SwiftUI

/// Tiny helper that looks up a string by key in the package's bundle,
/// honouring the "com.gibbio.marginalia.interfaceLanguage" AppStorage
/// override when set. SwiftUI's `Text("key")` uses `Bundle.main` by
/// default, which can't find resources inside a SwiftPM library, so any
/// view that wants to be localized goes through `T(...)`.
@inlinable
public func T(_ key: String, comment: String = "") -> String {
    let override = UserDefaults.standard.string(forKey: InterfaceLanguage.storageKey)
    let bundle = InterfaceLanguage.localizedBundle(forOverride: override)
    return NSLocalizedString(key, bundle: bundle, value: key, comment: comment)
}

public enum InterfaceLanguage {
    public static let storageKey = "com.gibbio.marginalia.interfaceLanguage"

    /// Supported UI locales. Order matters: displayed left-to-right in
    /// the Settings picker.
    public enum Code: String, CaseIterable, Hashable {
        case it
        case en
    }

    /// Native name of the language, for the picker label.
    public static func displayName(_ code: Code) -> String {
        switch code {
        case .it: return "Italiano"
        case .en: return "English"
        }
    }

    /// Returns the right `.lproj` bundle for the override, falling back
    /// to `Bundle.module` when the override is nil or points to a locale
    /// that isn't shipped.
    public static func localizedBundle(forOverride override: String?) -> Bundle {
        guard let code = override,
              let path = Bundle.module.path(forResource: code, ofType: "lproj"),
              let b = Bundle(path: path)
        else { return Bundle.module }
        return b
    }

    /// Switch the app's UI language. Writes the override key only —
    /// every `T(...)` call reads it at lookup time, and SwiftUI re-
    /// renders thanks to `@AppStorage` watching the same key. No
    /// relaunch needed. We deliberately *don't* touch `AppleLanguages`
    /// here: that would force a restart for system-bundle (`Bundle.main`)
    /// strings, but we exclusively go through the package bundle via
    /// the `localizedBundle(forOverride:)` path, so the system locale
    /// stays untouched.
    public static func apply(_ code: Code) {
        UserDefaults.standard.set(code.rawValue, forKey: storageKey)
    }
}
