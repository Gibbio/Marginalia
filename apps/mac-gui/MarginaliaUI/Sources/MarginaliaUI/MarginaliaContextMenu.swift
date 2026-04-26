import SwiftUI
#if canImport(AppKit)
import AppKit
#endif

/// Marginalia-styled context menu (right-click popover).
///
/// SwiftUI's built-in `.contextMenu` renders an `NSMenu` with the system
/// font + system selection colour — it follows light/dark, but it can't
/// be themed with Marginalia's serif typography or the accent hue.
/// This component is a hand-rolled replacement: a SwiftUI `popover`
/// triggered by an `NSView`-backed right-click catcher, populated by a
/// list of `MarginaliaMenuItem`s rendered with `Tokens.serif` + the
/// caller's `Accent`.
///
/// Usage:
/// ```swift
/// myView.marginaliaContextMenu(accent: accent) {
///     [
///         .init(label: T("foo"), icon: "play.fill",   action: { … }),
///         .init(label: T("bar"), icon: "trash",       isDestructive: true, action: { … }),
///     ]
/// }
/// ```

public struct MarginaliaMenuItem: Identifiable {
    public let id = UUID()
    public let label: String
    /// SF Symbol name for the leading icon. Empty string skips the icon.
    public let icon: String
    public let isDestructive: Bool
    public let isEnabled: Bool
    public let action: () -> Void

    public init(label: String,
                icon: String = "",
                isDestructive: Bool = false,
                isEnabled: Bool = true,
                action: @escaping () -> Void) {
        self.label = label
        self.icon = icon
        self.isDestructive = isDestructive
        self.isEnabled = isEnabled
        self.action = action
    }

    /// Sentinel for a divider row. The label is ignored. Built lazily
    /// so the static doesn't need `@MainActor` (the action closure is
    /// `Sendable`-incompatible by default; this avoids dragging
    /// concurrency annotations onto the call sites).
    public static var divider: MarginaliaMenuItem {
        MarginaliaMenuItem(label: "---", action: {})
    }
}

public extension View {
    /// Attach a Marginalia-styled context menu to this view.
    /// `items` is recomputed on every right-click so callers can use
    /// state (e.g. `isPlaying`) to vary the menu content.
    func marginaliaContextMenu(accent: Accent,
                               items: @escaping () -> [MarginaliaMenuItem]) -> some View {
        modifier(MarginaliaContextMenuModifier(accent: accent, items: items))
    }
}

private struct MarginaliaContextMenuModifier: ViewModifier {
    let accent: Accent
    let items: () -> [MarginaliaMenuItem]
    @State private var isShown = false
    @State private var snapshot: [MarginaliaMenuItem] = []

    func body(content: Content) -> some View {
        content
            #if canImport(AppKit)
            .background(
                RightClickCatcher(onRightClick: {
                    snapshot = items()
                    isShown = true
                })
            )
            #endif
            .popover(isPresented: $isShown, arrowEdge: .top) {
                MarginaliaMenuBody(items: snapshot, accent: accent,
                                   dismiss: { isShown = false })
            }
    }
}

#if canImport(AppKit)
/// Transparent NSView placed in `.background` of the target view; absorbs
/// right-mouse-down events without interfering with left-click / hover
/// gestures handled by SwiftUI on top.
private struct RightClickCatcher: NSViewRepresentable {
    let onRightClick: () -> Void

    func makeNSView(context: Context) -> NSView {
        let view = RightClickHandlerView()
        view.onRightClick = onRightClick
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        (nsView as? RightClickHandlerView)?.onRightClick = onRightClick
    }
}

private final class RightClickHandlerView: NSView {
    var onRightClick: (() -> Void)?

    override func rightMouseDown(with event: NSEvent) {
        onRightClick?()
    }

    /// Stay invisible to left-click hit testing — only consume right-click.
    /// Without this the catcher would steal the row's primary tap (open
    /// document, seek-to-chunk, etc.).
    override func hitTest(_ point: NSPoint) -> NSView? {
        if let event = NSApp.currentEvent,
           event.type == .rightMouseDown || event.type == .rightMouseUp {
            return self
        }
        return nil
    }
}
#endif

private struct MarginaliaMenuBody: View {
    let items: [MarginaliaMenuItem]
    let accent: Accent
    let dismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(items) { item in
                if item.label == "---" {
                    Divider()
                        .frame(height: 1)
                        .overlay(Tokens.line)
                        .padding(.vertical, 3)
                } else {
                    MarginaliaMenuRow(item: item, accent: accent, dismiss: dismiss)
                }
            }
        }
        .padding(.vertical, 4)
        .frame(minWidth: 200)
        .background(Tokens.paper)
    }
}

private struct MarginaliaMenuRow: View {
    let item: MarginaliaMenuItem
    let accent: Accent
    let dismiss: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: {
            item.action()
            dismiss()
        }) {
            HStack(spacing: 10) {
                if !item.icon.isEmpty {
                    Image(systemName: item.icon)
                        .font(.system(size: 11))
                        .frame(width: 14, alignment: .center)
                        .foregroundStyle(item.isDestructive
                                         ? Color.red.opacity(0.85)
                                         : (hovered ? accent.main : Tokens.textDim))
                } else {
                    Spacer().frame(width: 14)
                }
                Text(item.label)
                    .font(.serif(13))
                    .foregroundStyle(item.isEnabled
                                     ? (item.isDestructive
                                        ? Color.red.opacity(0.85)
                                        : Tokens.text)
                                     : Tokens.textFaint)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(hovered && item.isEnabled
                        ? accent.soft
                        : Color.clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!item.isEnabled)
        .onHover { hovered = $0 }
    }
}
