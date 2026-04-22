import CoreText
import Foundation

/// One-shot runtime registration of the `.ttf` files bundled under
/// `Sources/MarginaliaUI/Resources/fonts/`. Must be called **before** any
/// SwiftUI view that references the custom font names is constructed —
/// the preview executable does it in `main.swift`; the real `.app` should
/// do it in its `@main` struct `init()`.
///
/// Safe to call more than once: `CTFontManagerRegisterFontsForURL` returns
/// an error for the second call, which we silently swallow.
public enum Fonts {
    /// Register every font file in the bundle's font directory.
    public static func registerBundled() {
        let bundle = Bundle.module
        guard let urls = bundle.urls(forResourcesWithExtension: "ttf", subdirectory: "fonts") else {
            return
        }
        for url in urls {
            var unmanagedError: Unmanaged<CFError>?
            let ok = CTFontManagerRegisterFontsForURL(url as CFURL,
                                                     .process,
                                                     &unmanagedError)
            if !ok, let err = unmanagedError?.takeRetainedValue() {
                let code = CFErrorGetCode(err)
                // 105 = "font already registered" — benign on re-entry.
                if code != 105 {
                    let desc = CFErrorCopyDescription(err)
                    // swiftlint:disable:next print_on_release
                    print("[MarginaliaUI.Fonts] failed \(url.lastPathComponent): \(desc ?? "nil" as CFString)")
                }
            }
        }
    }
}
