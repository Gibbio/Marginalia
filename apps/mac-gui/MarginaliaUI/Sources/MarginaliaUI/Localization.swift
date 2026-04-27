import Foundation
import SwiftUI

/// Tiny helper that looks up a string by key in the package's bundle,
/// honouring the "com.gibbio.marginalia.interfaceLanguage" AppStorage
/// override when set. SwiftUI's `Text("key")` uses `Bundle.main` by
/// default, which can't find resources inside a SwiftPM library, so any
/// view that wants to be localized goes through `T(...)`.
///
/// Caches the resolved override+bundle pair so repeat calls during a
/// SwiftUI render pass don't hit `NSBundle.pathForResource` (the
/// `sample` profile showed bundle lookups dominating the Sidebar
/// re-render cost). Cleared by `InterfaceLanguage.apply(_:)`.
public func T(_ key: String, comment: String = "") -> String {
    let bundle = InterfaceLanguage.cachedBundle()
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

    /// Cache for the resolved bundle: `(override, bundle)`. `T(...)` is
    /// called many times per SwiftUI body re-evaluation; without this
    /// cache each call walks the `.lproj` directory via
    /// `NSBundle.pathForResource` and that file-system traversal showed
    /// up as a measurable chunk of the Sidebar redraw cost. The cache
    /// is read on every call but written only when the override
    /// changes (here or via `apply(_:)`), so the steady-state path is
    /// a single `UserDefaults.string(forKey:)` + dictionary-equivalent
    /// pointer compare.
    nonisolated(unsafe) private static var cache: (override: String?, bundle: Bundle)?
    private static let cacheLock = NSLock()

    /// Resolve the bundle for the current `interfaceLanguage` override,
    /// hitting the cache when the override hasn't changed since the
    /// last call. Returns the bundle to use for `NSLocalizedString`.
    public static func cachedBundle() -> Bundle {
        let override = UserDefaults.standard.string(forKey: storageKey)
        cacheLock.lock()
        defer { cacheLock.unlock() }
        if let cached = cache, cached.override == override {
            return cached.bundle
        }
        let resolved = localizedBundle(forOverride: override)
        cache = (override, resolved)
        return resolved
    }

    /// Drop the cached bundle so the next `T(...)` reloads it. Called
    /// from `apply(_:)` after the override flips; callers that mutate
    /// `UserDefaults` directly would also need to call this, but the
    /// supported API is `apply`.
    public static func invalidateBundleCache() {
        cacheLock.lock()
        cache = nil
        cacheLock.unlock()
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
        invalidateBundleCache()
    }
}
