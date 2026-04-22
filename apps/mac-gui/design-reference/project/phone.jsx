// Shared phone frame + status bar + bottom tab-bar used by all variants.
// Minimal, unbranded dark frame — 390 × 844 (iPhone-ish ratio).

function MPhone({ children, bg = '#0a0c10', statusColor = '#f3efe7', homeIndicator = true }) {
  return (
    <div style={{
      width: 390, height: 844, borderRadius: 54,
      background: bg, position: 'relative', overflow: 'hidden',
      boxShadow: '0 30px 70px rgba(0,0,0,0.35), 0 0 0 10px #17181c, 0 0 0 11px #2a2c32',
      color: statusColor,
    }}>
      {/* Dynamic-island-ish pill */}
      <div style={{
        position: 'absolute', top: 11, left: '50%', transform: 'translateX(-50%)',
        width: 112, height: 33, borderRadius: 22, background: '#000', zIndex: 60,
      }}/>
      {/* Status bar */}
      <div style={{
        position: 'absolute', top: 0, left: 0, right: 0, height: 54,
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '0 32px', paddingTop: 18, zIndex: 10, pointerEvents: 'none',
        fontFamily: 'Inter Tight, system-ui', fontSize: 15, fontWeight: 600,
        color: statusColor, letterSpacing: 0.2,
      }}>
        <span>9:41</span>
        <span style={{ display:'flex', alignItems:'center', gap:5 }}>
          <svg width="17" height="10" viewBox="0 0 17 10"><rect x="0" y="6" width="3" height="4" rx="0.5" fill={statusColor}/><rect x="4.5" y="4" width="3" height="6" rx="0.5" fill={statusColor}/><rect x="9" y="2" width="3" height="8" rx="0.5" fill={statusColor}/><rect x="13.5" y="0" width="3" height="10" rx="0.5" fill={statusColor}/></svg>
          <svg width="22" height="10" viewBox="0 0 22 10"><rect x="0.5" y="0.5" width="18" height="9" rx="2.5" fill="none" stroke={statusColor} strokeOpacity="0.5"/><rect x="2" y="2" width="13" height="6" rx="1" fill={statusColor}/><rect x="20" y="3.5" width="1.5" height="3" rx="0.5" fill={statusColor} fillOpacity="0.5"/></svg>
        </span>
      </div>
      {/* content */}
      <div style={{ position:'absolute', inset:0 }}>{children}</div>
      {/* home indicator */}
      {homeIndicator && (
        <div style={{
          position: 'absolute', bottom: 9, left: '50%', transform: 'translateX(-50%)',
          width: 134, height: 5, borderRadius: 3,
          background: 'rgba(255,255,255,0.85)', zIndex: 80,
        }}/>
      )}
    </div>
  );
}

// An abstract placeholder SVG (striped) — never draw illustrations, always use this.
function MPlaceholder({ w = '100%', h = 80, label = 'placeholder', tone = 'rgba(255,255,255,0.08)', stroke = 'rgba(255,255,255,0.14)', radius = 12 }) {
  const id = 'stripe-' + Math.random().toString(36).slice(2, 8);
  return (
    <div style={{ width: w, height: h, borderRadius: radius, overflow: 'hidden', position: 'relative', background: tone }}>
      <svg width="100%" height="100%" style={{ display:'block' }}>
        <defs>
          <pattern id={id} patternUnits="userSpaceOnUse" width="8" height="8" patternTransform="rotate(45)">
            <line x1="0" y1="0" x2="0" y2="8" stroke={stroke} strokeWidth="1" />
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill={`url(#${id})`} />
      </svg>
      <div style={{
        position:'absolute', inset:0, display:'flex', alignItems:'center', justifyContent:'center',
        fontFamily: 'JetBrains Mono, monospace', fontSize: 10, letterSpacing: 0.5,
        color: 'rgba(255,255,255,0.45)', textTransform: 'lowercase',
      }}>{label}</div>
    </div>
  );
}

Object.assign(window, { MPhone, MPlaceholder });
