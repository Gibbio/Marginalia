// Marginalia — macOS desktop (Inchiostro) con link visibili nota↔chunk + Tweaks.

// ── TWEAKS persisted defaults ──
const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "showChunkNumbers": true,
  "linkStyle": "curve",
  "glowActiveChunk": true,
  "accentHue": 250
}/*EDITMODE-END*/;

const d = {
  bg: '#0f0e10',
  bg2: '#161315',
  paper: '#1a1815',
  text: '#efe5cf',
  textDim: 'rgba(239,229,207,0.58)',
  textFaint: 'rgba(239,229,207,0.28)',
  textGhost: 'rgba(239,229,207,0.14)',
  line: 'rgba(239,229,207,0.08)',
  lineSoft: 'rgba(239,229,207,0.04)',
  serif: "'Cormorant Garamond', Georgia, serif",
  sans: "'Inter Tight', system-ui, sans-serif",
  mono: "'JetBrains Mono', monospace",
};

// Accent based on tweak hue
function makeAccent(hue) {
  return {
    accent: `oklch(0.7 0.14 ${hue})`,
    accentDeep: `oklch(0.5 0.15 ${hue})`,
    accentSoft: `oklch(0.7 0.14 ${hue} / 0.18)`,
    accentGlow: `oklch(0.7 0.14 ${hue} / 0.32)`,
  };
}

// ── Anchored text (chunks) ──
// Each chunk has id, text, and notes list. Linking layer uses bounding rects.
const CHUNKS = [
  { id: 'c1', text: "Il tempo, nell'alta montagna, non è il tempo della pianura. Si dilata, si contrae, talvolta sembra fermarsi del tutto, come se l'aria rarefatta ne modificasse la sostanza stessa." },
  { id: 'c2', text: "Hans Castorp osservava la neve cadere oltre il vetro, e pensava — non senza un certo stupore — che erano passate già sette settimane dal suo arrivo, sette settimane che egli aveva contato come giorni, e che ora, al solo ricordarle, gli parevano un istante." },
  { id: 'c3', text: "Ma forse, pensò, non è la durata a contare, quanto la qualità del tempo vissuto. Una settimana in pianura poteva dissolversi senza lasciare traccia; mentre un solo pomeriggio quassù, trascorso a guardare il cielo cambiare colore sopra i larici, poteva pesare come un anno intero." },
  { id: 'c4', text: "Joachim, suo cugino, rideva di queste sue meditazioni. «Tu filosofeggi, Hans», diceva, «come fanno tutti i principianti. Tra sei mesi avrai smesso»." },
  { id: 'c5', text: "Qui, in alto, dove persino il «sanatorio» pareva sospeso tra due cieli, le parole perdevano il loro peso quotidiano e ne assumevano uno nuovo, più lento, più pieno." },
  { id: 'c6', text: "E pure nel ridere c'era una malinconia sottile, perché Joachim stesso — il giovane ufficiale che sognava di tornare al reggimento — aveva smesso, a forza, di contare i giorni." },
];

const NOTES = [
  { id:'n0', chunkId:'c2', when:'poco fa', quote:"…sette settimane dal suo arrivo…",
    body:"Qui il tempo è trattato come uno spazio che si può attraversare. Rendilo più sensoriale — fai sentire la lentezza.", dur:'0:14', status:'rielaborato', live:true },
  { id:'n1', chunkId:'c5', when:'10 min fa', quote:"«sanatorio»",
    body:"metafora o luogo reale? Mann gioca sul doppio senso per tutta la prima parte.", dur:'0:21', status:'' },
  { id:'n2', chunkId:'c3', when:'ieri', quote:"il cielo cambiare colore",
    body:"Confronta con Proust — la stessa attenzione al dettaglio atmosferico, ma qui più silenziosa.", dur:'0:33', status:'applicato' },
  { id:'n3', chunkId:'c4', when:'2 gg fa', quote:"«Tu filosofeggi, Hans»",
    body:"Joachim come contrappunto razionale. Mantieni la leggerezza dello scherzo.", dur:'0:11', status:'' },
];

// ── Tweaks context ──
const TweakCtx = React.createContext(TWEAK_DEFAULTS);

// ── Hover context for note↔chunk linking ──
const HoverCtx = React.createContext({ hoverId: null, setHoverId: () => {} });

function DarkTraffic() {
  return (
    <div style={{ display:'flex', gap: 8 }}>
      {['#ff5f57','#febc2e','#28c840'].map((c,i)=>(
        <div key={i} style={{ width:12, height:12, borderRadius:'50%', background:c,
          boxShadow:'inset 0 0 0 0.5px rgba(0,0,0,0.25)' }}/>
      ))}
    </div>
  );
}

function Ambient({ top, left, size=500, color='oklch(0.3 0.13 250 / 0.22)' }) {
  return (
    <div style={{
      position:'absolute', top, left, width:size, height:size, borderRadius:'50%',
      background:`radial-gradient(circle, ${color}, transparent 65%)`,
      transform:'translate(-50%,-50%)', filter:'blur(28px)', pointerEvents:'none', zIndex:0,
    }}/>
  );
}

function Sidebar({ a, onOpenSettings }) {
  const lib = [
    { t:'La montagna incantata', s:'Thomas Mann', pct:34, active:true, notes: 12 },
    { t:'Lettera a Giulia — v4', s:'bozza', pct:88, notes: 1 },
    { t:'Appunti sul Simposio', s:'Platone', pct:12, notes: 4 },
    { t:'Note al convegno', s:'bozza', pct:56, notes: 7 },
    { t:'Il giovane Holden', s:'J.D. Salinger', pct:0, notes: 0 },
    { t:'Paesaggi della mente', s:'saggio · v2', pct:22, notes: 3 },
  ];
  return (
    <div style={{
      width: 264, flexShrink: 0, background: d.bg2,
      borderRight: `1px solid ${d.line}`,
      display:'flex', flexDirection:'column', position:'relative', zIndex: 2,
    }}>
      <div style={{ padding: '14px 18px 14px 82px', height: 52, boxSizing:'border-box',
        borderBottom:`1px solid ${d.line}`, display:'flex', alignItems:'center',
        justifyContent:'space-between', gap: 8,
      }}>
        <div style={{ display:'flex', alignItems:'center', gap: 10, minWidth: 0 }}>
          <div
            onClick={onOpenSettings}
            title="Impostazioni"
            style={{ width:22, height:22, borderRadius:6, border:`1px solid ${d.textGhost}`,
              display:'flex', alignItems:'center', justifyContent:'center',
              color: d.textDim, cursor: 'pointer', flexShrink: 0,
            }}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3"/>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h0a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v0a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
            </svg>
          </div>
          <div style={{ fontFamily: d.serif, fontSize: 18, fontStyle:'italic', color: d.text }}>Marginalia</div>
        </div>
        <div style={{ width:22, height:22, borderRadius:6, border:`1px solid ${d.textGhost}`,
          display:'flex', alignItems:'center', justifyContent:'center',
          fontSize: 14, color: d.textDim, cursor: 'pointer', flexShrink: 0,
        }}>+</div>
      </div>

      <div style={{ padding:'12px 14px 10px' }}>
        <div style={{
          display:'flex', alignItems:'center', gap: 8, padding:'7px 10px', borderRadius: 8,
          background:'rgba(255,255,255,0.03)', border:`1px solid ${d.line}`,
        }}>
          <svg width="12" height="12" viewBox="0 0 12 12"><circle cx="5" cy="5" r="3.5" stroke={d.textFaint} strokeWidth="1.2" fill="none"/><path d="M7.5 7.5L10 10" stroke={d.textFaint} strokeWidth="1.2" strokeLinecap="round"/></svg>
          <div style={{ fontFamily: d.sans, fontSize: 12, color: d.textFaint, flex:1 }}>cerca o di'…</div>
          <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textFaint, padding:'2px 5px',
            borderRadius: 4, background:'rgba(255,255,255,0.04)' }}>⌘K</div>
        </div>
      </div>

      <div style={{ overflowY:'auto', flex: 1 }} className="no-scrollbar">
        <SideSection title="Raccolte">
          <SideRow label="Tutto" count={47}/>
          <SideRow label="In ascolto" count={3} active a={a}/>
          <SideRow label="Bozze" count={4}/>
          <SideRow label="Archivio" count={28}/>
        </SideSection>
        <SideSection title="Libreria">
          {lib.map((it, i)=><LibRow key={i} {...it} a={a}/>)}
        </SideSection>
      </div>

      <div style={{ padding:'12px 14px', borderTop:`1px solid ${d.line}`,
        display:'flex', alignItems:'center', gap: 10 }}>
        <div style={{ width:26, height:26, borderRadius:'50%', border:`1px solid ${d.textGhost}`,
          display:'flex', alignItems:'center', justifyContent:'center' }}>
          <div style={{ width:5, height:5, borderRadius:'50%', background: a.accent,
            boxShadow:`0 0 10px ${a.accent}` }}/>
        </div>
        <div>
          <div style={{ fontFamily: d.serif, fontSize: 13, color: d.text, fontStyle:'italic' }}>Voce: Elena</div>
          <div style={{ fontFamily: d.mono, fontSize: 9, color: d.textFaint, marginTop:1 }}>IT · 1.0×</div>
        </div>
        <div style={{ flex:1 }}/>
        <div style={{ display:'flex', alignItems:'center', gap:1.5, height:14 }}>
          {[4,8,5,10,6,9,4,7,11,5].map((h,i)=>(
            <div key={i} style={{ width:1.5, height:h, background: a.accent, opacity:0.4+(i%3)*0.2 }}/>
          ))}
        </div>
      </div>
    </div>
  );
}

function SideSection({ title, children }) {
  return (
    <div style={{ padding:'6px 0 14px' }}>
      <div style={{ padding:'8px 18px 6px', fontFamily: d.mono, fontSize: 10,
        color: d.textFaint, letterSpacing: 1.5 }}>{title.toLowerCase()}</div>
      {children}
    </div>
  );
}

function SideRow({ label, count, active, a }) {
  return (
    <div style={{
      display:'flex', alignItems:'center', margin:'0 8px', padding:'6px 10px', borderRadius:6,
      background: active ? 'rgba(255,255,255,0.04)' : 'transparent',
      borderLeft: active ? `2px solid ${a?.accent || '#fff'}` : '2px solid transparent',
      fontFamily: d.serif, fontSize: 14, fontStyle: active?'italic':'normal',
      color: active ? d.text : d.textDim,
    }}>
      <div style={{ flex:1 }}>{label}</div>
      <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textFaint }}>{count}</div>
    </div>
  );
}

function LibRow({ t, s, pct, active, notes, a }) {
  return (
    <div style={{ display:'flex', gap:10, alignItems:'flex-start',
      margin:'0 8px', padding:'8px 10px', borderRadius:6,
      background: active?'rgba(255,255,255,0.05)':'transparent',
      position:'relative', marginBottom:2,
    }}>
      {active && (<div style={{ position:'absolute', left:0, top:10, bottom:10, width:2,
        background: a.accent, borderRadius:2, boxShadow:`0 0 10px ${a.accentGlow}` }}/>)}
      <div style={{ flex:1, minWidth:0 }}>
        <div style={{ fontFamily: d.serif, fontSize: 15, color: active?d.text:d.textDim,
          lineHeight:1.2, marginBottom:2, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap',
          fontStyle: active?'italic':'normal',
        }}>{t}</div>
        <div style={{ fontFamily: d.mono, fontSize: 9, color: d.textFaint, marginBottom:5,
          letterSpacing: 0.3,
        }}>{s} · {pct}%</div>
        {pct>0 && (
          <div style={{ height:1, background:'rgba(239,229,207,0.08)', position:'relative' }}>
            <div style={{ position:'absolute', left:0, top:0, height:1, width:`${pct}%`,
              background: active ? a.accent : 'rgba(239,229,207,0.35)' }}/>
          </div>
        )}
      </div>
      {notes>0 && <div style={{ fontFamily: d.mono, fontSize: 9,
        color: active ? a.accent : d.textFaint, paddingTop:2 }}>{notes}</div>}
    </div>
  );
}

function Toolbar({ a }) {
  return (
    <div style={{ height:52, padding:'0 20px', display:'flex', alignItems:'center',
      justifyContent:'space-between', borderBottom:`1px solid ${d.line}`,
      position:'relative', zIndex: 3,
    }}>
      <div style={{ display:'flex', alignItems:'center', gap: 14 }}>
        <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textFaint,
          letterSpacing: 1.5, textTransform:'uppercase' }}>In ascolto</div>
        <div style={{ fontFamily: d.serif, fontSize: 17, fontStyle:'italic', color: d.text }}>La montagna incantata</div>
        <div style={{ width:3, height:3, borderRadius:'50%', background: d.textFaint }}/>
        <div style={{ fontFamily: d.serif, fontSize: 14, color: d.textDim }}>capitolo III · p. 47</div>
      </div>

      <div style={{ position:'absolute', left:'50%', top:10, transform:'translateX(-50%)',
        display:'flex', alignItems:'center', gap:10, padding:'6px 8px 6px 14px',
        borderRadius:999, background:'rgba(255,255,255,0.04)', border:`1px solid ${d.line}`,
      }}>
        <div style={{ fontFamily: d.mono, fontSize:10, color: d.textFaint }}>04:23</div>
        <div style={{ display:'flex', alignItems:'center', gap:1.5, height:18 }}>
          {Array.from({length:44}).map((_,i)=>{
            const h = 3 + Math.abs(Math.sin(i*0.55))*11 + (i%3)*2;
            const played = i<15;
            return <div key={i} style={{ width:1.5, height:h, borderRadius:1,
              background: played?a.accent:'rgba(239,229,207,0.22)',
              opacity: played?0.9:0.5, boxShadow: played?`0 0 3px ${a.accent}`:'none' }}/>;
          })}
        </div>
        <div style={{ fontFamily: d.mono, fontSize:10, color: d.textFaint }}>12:03</div>
        <div style={{ width:1, height:14, background:d.line, margin:'0 4px' }}/>
        <div style={{ width:28, height:28, borderRadius:'50%', background: a.accent,
          display:'flex', alignItems:'center', justifyContent:'center',
          boxShadow:`0 0 16px ${a.accentGlow}` }}>
          <div style={{ display:'flex', gap:2.5 }}>
            <div style={{ width:2.5, height:10, background:'#0f0e10' }}/>
            <div style={{ width:2.5, height:10, background:'#0f0e10' }}/>
          </div>
        </div>
        <div style={{ fontFamily: d.mono, fontSize:10, color: d.textDim,
          padding:'2px 6px', borderRadius:4, background:'rgba(255,255,255,0.04)' }}>1.0×</div>
      </div>

      <div style={{ display:'flex', alignItems:'center', gap:8 }}>
        <div style={{ display:'inline-flex', alignItems:'center', gap:7,
          padding:'5px 11px', borderRadius:999, background:'rgba(255,255,255,0.03)',
          border:`1px solid ${d.line}`,
        }}>
          <div style={{ width:5, height:5, borderRadius:'50%', background: a.accent,
            boxShadow:`0 0 8px ${a.accent}` }}/>
          <div style={{ fontFamily: d.serif, fontStyle:'italic', fontSize:13, color: d.textDim }}>ascolto voce</div>
        </div>
      </div>
    </div>
  );
}

function ReadingColumn({ a, chunkRefs }) {
  const tw = React.useContext(TweakCtx);
  const { hoverId } = React.useContext(HoverCtx);
  const anchoredIds = new Set(NOTES.map(n=>n.chunkId));

  return (
    <div data-reading-col style={{ flex:1, position:'relative', overflow:'hidden',
      display:'flex', flexDirection:'column',
    }}>
      <Ambient top="45%" left="50%" size={700} color={`oklch(0.35 0.14 ${tw.accentHue} / 0.18)`} />
      <div className="no-scrollbar" style={{ flex:1, overflow:'auto', padding:'54px 88px 60px',
        position:'relative', zIndex:1,
      }}>
        <div style={{ marginBottom: 44 }}>
          <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textFaint,
            letterSpacing: 2, textTransform:'uppercase', marginBottom: 12 }}>Der Zauberberg · parte seconda</div>
          <h1 style={{ margin:0, fontFamily: d.serif, fontSize: 46, lineHeight: 1.05,
            color: d.text, letterSpacing:-0.6, fontWeight: 500,
          }}>Capitolo III <br/><em style={{ color: d.textDim, fontWeight:400 }}>il tempo in montagna</em></h1>
        </div>

        <div style={{ fontFamily: d.serif, fontSize: 21, lineHeight: 1.75,
          color: d.textDim, maxWidth: 640, position:'relative',
        }}>
          {CHUNKS.map((c, i) => {
            const hasNote = anchoredIds.has(c.id);
            const isHover = hoverId === c.id;
            const dimmed = hoverId && !isHover; // when hovering a note, fade everything else
            return (
              <p key={c.id} style={{ margin:0, marginBottom: 22, position:'relative' }}>
                {/* gutter chunk number */}
                {tw.showChunkNumbers && hasNote && (
                  <span style={{
                    position:'absolute', left:-42, top:6,
                    width: 24, height: 24, borderRadius:'50%',
                    border:`1px solid ${isHover ? a.accent : d.textGhost}`,
                    color: isHover ? a.accent : d.textFaint,
                    background: isHover ? a.accentSoft : 'transparent',
                    display:'flex', alignItems:'center', justifyContent:'center',
                    fontFamily: d.mono, fontSize: 10,
                    opacity: dimmed ? 0.3 : 1,
                    boxShadow: isHover ? `0 0 16px ${a.accentGlow}` : 'none',
                    transition: 'all 0.2s ease',
                  }}>{NOTES.findIndex(n=>n.chunkId===c.id)+1}</span>
                )}

                <span
                  ref={el => { if (el) chunkRefs.current[c.id] = el; }}
                  data-chunk={c.id}
                  style={{
                    color: isHover ? d.text : (dimmed ? 'rgba(239,229,207,0.25)' : d.textDim),
                    background: isHover
                      ? `linear-gradient(180deg, transparent 55%, ${a.accentSoft} 55%, ${a.accentSoft} 94%, transparent 94%)`
                      : 'none',
                    boxShadow: isHover && tw.glowActiveChunk
                      ? `0 0 0 1px oklch(0.7 0.14 ${tw.accentHue} / 0.22)` : 'none',
                    borderRadius: 2, transition: 'color 0.25s ease, background 0.2s ease', cursor: hasNote ? 'pointer' : 'default',
                }}>
                  {c.text}
                </span>
              </p>
            );
          })}
        </div>
      </div>

      <div style={{ position:'absolute', bottom:20, left:'50%', transform:'translateX(-50%)',
        zIndex: 5, display:'inline-flex', alignItems:'center', gap:8,
        padding:'8px 14px', borderRadius:999,
        background:'rgba(26,24,21,0.82)', backdropFilter:'blur(18px)',
        border:`1px solid ${d.line}`, fontFamily: d.serif, fontSize: 13, fontStyle:'italic',
        color: d.textDim,
      }}>
        <div style={{ display:'flex', alignItems:'center', gap:1.5, height:12 }}>
          {[3,6,4,8,5,7,4,5,7,4,3].map((h,i)=>(
            <div key={i} style={{ width:1.5, height:h, background:a.accent, opacity:0.4+(i%3)*0.2 }}/>
          ))}
        </div>
        <span>di' qualcosa — o <span style={{ color: a.accent }}>"pausa"</span></span>
      </div>
    </div>
  );
}

function MarginPanel({ a, noteRefs }) {
  const { setHoverId } = React.useContext(HoverCtx);
  return (
    <div style={{
      width: 340, flexShrink:0, borderLeft:`1px solid ${d.line}`,
      background: d.bg, position:'relative', zIndex: 2,
      display:'flex', flexDirection:'column',
    }}>
      <div style={{ padding:'14px 18px', borderBottom:`1px solid ${d.line}`,
        display:'flex', alignItems:'center', justifyContent:'space-between',
        height: 52, boxSizing:'border-box',
      }}>
        <div>
          <div style={{ fontFamily: d.mono, fontSize:10, color: d.textFaint,
            letterSpacing: 1.5, textTransform:'uppercase', marginBottom: 2 }}>Marginalia</div>
          <div style={{ fontFamily: d.serif, fontStyle:'italic', fontSize:15, color: d.text }}>
            {NOTES.length} note in questo capitolo
          </div>
        </div>
        <div style={{ fontFamily: d.mono, fontSize:10, color: d.textFaint,
          padding:'3px 7px', borderRadius:4, border:`1px solid ${d.line}` }}>⌥M</div>
      </div>

      <div className="no-scrollbar" style={{ flex:1, overflow:'auto' }}>
        {NOTES.map((n, i) => n.live ? (
          <LiveNote key={n.id} n={n} idx={i+1} a={a} noteRefs={noteRefs} setHoverId={setHoverId}/>
        ) : (
          <MarginNote key={n.id} n={n} idx={i+1} a={a} noteRefs={noteRefs} setHoverId={setHoverId}/>
        ))}
      </div>

      <div style={{ padding:'12px 18px', borderTop:`1px solid ${d.line}`,
        display:'flex', alignItems:'center', gap:10,
      }}>
        <div style={{ width:24, height:24, borderRadius:6, background:'rgba(255,255,255,0.04)',
          display:'flex', alignItems:'center', justifyContent:'center' }}>
          <div style={{ width:5, height:5, borderRadius:'50%', background: a.accent }}/>
        </div>
        <div style={{ flex:1, fontFamily: d.serif, fontStyle:'italic', fontSize:13, color: d.textDim }}>
          passa sopra una nota per vedere il chunk collegato
        </div>
      </div>
    </div>
  );
}

function LiveNote({ n, idx, a, noteRefs, setHoverId }) {
  return (
    <div
      ref={el => { if (el) noteRefs.current[n.id] = el; }}
      data-note={n.id}
      onMouseEnter={()=>setHoverId(n.chunkId)}
      onMouseLeave={()=>setHoverId(null)}
      style={{
        margin:'12px 14px', padding:14, borderRadius:8,
        background:`linear-gradient(180deg, oklch(0.3 0.14 250 / 0.28), rgba(26,24,21,0.4))`,
        border:`1px solid ${a.accent}`,
        boxShadow:`0 0 24px ${a.accentGlow}`, position:'relative',
      }}>
      <div style={{ display:'flex', alignItems:'center', gap:8, marginBottom: 10 }}>
        <div style={{
          width: 22, height: 22, borderRadius:'50%',
          border:`1px solid ${a.accent}`, color: a.accent,
          display:'flex', alignItems:'center', justifyContent:'center',
          fontFamily: d.mono, fontSize: 10,
          boxShadow:`0 0 10px ${a.accentGlow}`,
        }}>{idx}</div>
        <div style={{ fontFamily: d.mono, fontSize:9, letterSpacing:1.5,
          color: a.accent, textTransform:'uppercase' }}>stai parlando</div>
        <div style={{ flex:1 }}/>
        <div style={{ fontFamily: d.mono, fontSize:9, color: d.textDim }}>0:14</div>
      </div>
      <div style={{ fontFamily: d.serif, fontSize: 15, fontStyle:'italic',
        lineHeight: 1.5, color: d.text, marginBottom: 12,
      }}>
        "Qui il tempo è trattato come uno spazio che si può <span style={{ color: a.accent }}>attraversare</span>
        <span style={{ display:'inline-block', width:1.5, height:14, background: a.accent,
          marginLeft:3, verticalAlign:'middle', boxShadow:`0 0 6px ${a.accent}` }}/>
      </div>
      <div style={{ display:'flex', alignItems:'center', gap:1.5, height:22, marginBottom:4 }}>
        {Array.from({length:56}).map((_,i)=>{
          const h = 3 + Math.abs(Math.sin(i*0.55))*16 + (i%4)*3;
          return <div key={i} style={{ width:1.5, height:h, background:a.accent,
            opacity: 0.3+Math.abs(Math.sin(i*0.3))*0.6 }}/>;
        })}
      </div>
      <div style={{ fontFamily: d.serif, fontStyle:'italic', fontSize: 12, color: d.textFaint }}>
        di' <span style={{ color: d.textDim }}>"fatto"</span> per salvare · <span style={{ color: d.textDim }}>"rielabora"</span> per riscrivere
      </div>
    </div>
  );
}

function MarginNote({ n, idx, a, noteRefs, setHoverId }) {
  const [hover, setHover] = React.useState(false);
  return (
    <div
      ref={el => { if (el) noteRefs.current[n.id] = el; }}
      data-note={n.id}
      onMouseEnter={()=>{ setHover(true); setHoverId(n.chunkId); }}
      onMouseLeave={()=>{ setHover(false); setHoverId(null); }}
      style={{
        padding:'14px 18px 14px 16px', borderBottom:`1px solid ${d.lineSoft}`,
        position:'relative', cursor:'pointer',
        background: hover ? 'rgba(239,229,207,0.02)' : 'transparent',
        borderLeft: hover ? `2px solid ${a.accent}` : `2px solid transparent`,
        transition: 'all 0.15s ease',
      }}>
      <div style={{ display:'flex', alignItems:'center', gap:8, marginBottom:6 }}>
        <div style={{
          width: 20, height: 20, borderRadius:'50%',
          border:`1px solid ${hover ? a.accent : d.textGhost}`,
          color: hover ? a.accent : d.textFaint,
          display:'flex', alignItems:'center', justifyContent:'center',
          fontFamily: d.mono, fontSize: 9,
        }}>{idx}</div>
        <div style={{ fontFamily: d.mono, fontSize:9, color: d.textFaint }}>{n.when}</div>
        <div style={{ flex:1 }}/>
        {n.status && (
          <div style={{ fontFamily: d.mono, fontSize:8, letterSpacing:1,
            color: a.accent, textTransform:'uppercase',
            padding:'2px 5px', borderRadius:3,
            background:`oklch(0.7 0.14 250 / 0.08)`,
            border:`1px solid oklch(0.7 0.14 250 / 0.18)`,
          }}>{n.status}</div>
        )}
      </div>
      <div style={{ fontFamily: d.serif, fontStyle:'italic', fontSize:12,
        lineHeight: 1.4, color: d.textDim, marginBottom: 8, paddingLeft: 8,
        borderLeft:`1px solid ${d.line}`,
      }}>{n.quote}</div>
      <div style={{ fontFamily: d.serif, fontSize:14, lineHeight:1.5, color: d.text,
        marginBottom: 10,
      }}>{n.body}</div>
      <div style={{ display:'flex', alignItems:'center', gap:10 }}>
        <div style={{ width:20, height:20, borderRadius:'50%',
          border:`1px solid ${d.textGhost}`,
          display:'flex', alignItems:'center', justifyContent:'center', flexShrink:0 }}>
          <div style={{ width:0, height:0, borderLeft:`5px solid ${d.textDim}`,
            borderTop:'3.5px solid transparent', borderBottom:'3.5px solid transparent',
            marginLeft:1.5 }}/>
        </div>
        <div style={{ display:'flex', alignItems:'center', gap:1.5, height:10, flex:1 }}>
          {Array.from({length:40}).map((_,i)=>{
            const h = 2 + Math.abs(Math.sin(i*0.5 + n.id.length))*7;
            return <div key={i} style={{ width:1.5, height:h, background:d.textDim,
              opacity:0.3+(i%3)*0.2 }}/>;
          })}
        </div>
        <div style={{ fontFamily: d.mono, fontSize:9, color: d.textFaint }}>{n.dur}</div>
      </div>
    </div>
  );
}

// ── Linking overlay: SVG that draws a curve from chunk→note on hover ──
function LinkOverlay({ chunkRefs, noteRefs, containerRef, a }) {
  const tw = React.useContext(TweakCtx);
  const { hoverId } = React.useContext(HoverCtx);
  const [, force] = React.useReducer(x=>x+1, 0);

  React.useEffect(() => {
    const onScroll = () => force();
    const nodes = containerRef.current?.querySelectorAll('.no-scrollbar') || [];
    nodes.forEach(n => n.addEventListener('scroll', onScroll, { passive:true }));
    return () => nodes.forEach(n => n.removeEventListener('scroll', onScroll));
  }, []);

  if (tw.linkStyle === 'none' || !hoverId) return null;
  const chunk = chunkRefs.current[hoverId];
  const note = NOTES.find(n => n.chunkId === hoverId);
  if (!chunk || !note) return null;
  const noteEl = noteRefs.current[note.id];
  if (!noteEl || !containerRef.current) return null;

  // Use the LAST line of the chunk's text so the link exits the end of the passage,
  // not the middle of a multi-line span (which can visually overlap neighbor paragraphs).
  const rects = chunk.getClientRects();
  const cr = rects[rects.length - 1] || chunk.getBoundingClientRect();
  const nr = noteEl.getBoundingClientRect();
  const wr = containerRef.current.getBoundingClientRect();

  // Anchor at the right edge of the reading column (not the end of the line text,
  // which can be mid-column on a short final line). This reads as "this passage → note".
  const readingCol = containerRef.current.querySelector('[data-reading-col]');
  const colRect = readingCol ? readingCol.getBoundingClientRect() : null;
  const x1 = (colRect ? colRect.right : cr.right) - wr.left - 8;
  const y1 = cr.top + cr.height/2 - wr.top;
  const x2 = nr.left - wr.left;
  const y2 = nr.top + 24 - wr.top;

  const path = tw.linkStyle === 'curve'
    ? `M ${x1} ${y1} C ${x1 + (x2-x1)*0.5} ${y1}, ${x2 - (x2-x1)*0.5} ${y2}, ${x2} ${y2}`
    : `M ${x1} ${y1} L ${x2} ${y2}`;

  return (
    <svg style={{ position:'absolute', inset:0, pointerEvents:'none', zIndex: 6 }}
      width="100%" height="100%">
      <defs>
        <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="2" result="b"/>
          <feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge>
        </filter>
      </defs>
      <path d={path} stroke={a.accent} strokeWidth="1" fill="none"
        strokeDasharray={tw.linkStyle==='dashed' ? '4 4' : 'none'}
        opacity="0.85" filter="url(#glow)"/>
      <circle cx={x1} cy={y1} r="3" fill={a.accent} filter="url(#glow)"/>
      <circle cx={x2} cy={y2} r="3" fill={a.accent} filter="url(#glow)"/>
    </svg>
  );
}

function Window({ initialMode = 'reading' }) {
  const [tweaks, setTweaks] = React.useState(TWEAK_DEFAULTS);
  const [hoverId, setHoverId] = React.useState(null);
  const [mode, setMode] = React.useState(initialMode);
  const chunkRefs = React.useRef({});
  const noteRefs = React.useRef({});
  const containerRef = React.useRef(null);
  const a = makeAccent(tweaks.accentHue);

  return (
    <TweakCtx.Provider value={tweaks}>
      <HoverCtx.Provider value={{ hoverId, setHoverId }}>
        <div style={{
          width: 1440, height: 900, borderRadius: 14, overflow:'hidden',
          background: d.bg, position:'relative',
          boxShadow:'0 0 0 0.5px rgba(255,255,255,0.06), 0 30px 80px rgba(0,0,0,0.5)',
          display:'flex', flexDirection:'column',
        }}>
          <div style={{ height:36, background:d.bg2, borderBottom:`1px solid ${d.line}`,
            display:'flex', alignItems:'center', padding:'0 16px', position:'relative', zIndex:3,
          }}>
            <DarkTraffic/>
            <div style={{ position:'absolute', left:'50%', top:'50%', transform:'translate(-50%,-50%)',
              fontFamily: d.serif, fontSize:13, fontStyle:'italic', color: d.textDim,
            }}>Marginalia{mode==='settings' ? ' — Impostazioni' : ' — La montagna incantata'}</div>
          </div>

          <div ref={containerRef} style={{ display:'flex', flex:1, position:'relative', overflow:'hidden' }}>
            <Sidebar a={a} onOpenSettings={()=>setMode(m => m==='settings' ? 'reading' : 'settings')}/>
            <div style={{ flex:1, display:'flex', flexDirection:'column', position:'relative', overflow:'hidden' }}>
              {mode === 'settings' ? (
                <SettingsView a={a} onClose={()=>setMode('reading')}/>
              ) : (
                <>
                  <Toolbar a={a}/>
                  <div style={{ display:'flex', flex:1, overflow:'hidden', position:'relative' }}>
                    <ReadingColumn a={a} chunkRefs={chunkRefs}/>
                    <MarginPanel a={a} noteRefs={noteRefs}/>
                    <LinkOverlay chunkRefs={chunkRefs} noteRefs={noteRefs} containerRef={containerRef} a={a}/>
                  </div>
                </>
              )}
            </div>
          </div>
        </div>

        <TweaksPanel tweaks={tweaks} setTweaks={setTweaks}/>
      </HoverCtx.Provider>
    </TweakCtx.Provider>
  );
}

// ── Tweaks panel ──
function TweaksPanel({ tweaks, setTweaks }) {
  const [visible, setVisible] = React.useState(false);

  React.useEffect(() => {
    const onMsg = (e) => {
      if (e.data?.type === '__activate_edit_mode') setVisible(true);
      if (e.data?.type === '__deactivate_edit_mode') setVisible(false);
    };
    window.addEventListener('message', onMsg);
    window.parent.postMessage({ type: '__edit_mode_available' }, '*');
    return () => window.removeEventListener('message', onMsg);
  }, []);

  const set = (k, v) => {
    const next = { ...tweaks, [k]: v };
    setTweaks(next);
    window.parent.postMessage({ type: '__edit_mode_set_keys', edits: { [k]: v } }, '*');
  };

  if (!visible) return null;

  return (
    <div style={{
      position:'fixed', bottom: 20, right: 20, width: 300, zIndex: 9999,
      background: 'rgba(20,18,20,0.95)', backdropFilter:'blur(18px)',
      border:'1px solid rgba(239,229,207,0.12)', borderRadius: 14,
      padding: 18, color: '#efe5cf',
      fontFamily: "'Inter Tight', system-ui",
      boxShadow:'0 20px 60px rgba(0,0,0,0.5)',
    }}>
      <div style={{ fontFamily:"'Cormorant Garamond', serif", fontSize: 22,
        fontStyle:'italic', marginBottom: 4 }}>Tweaks</div>
      <div style={{ fontSize: 11, opacity: 0.55, marginBottom: 16 }}>
        modifica collegamenti e stile — passa sopra una nota per testare
      </div>

      <Row label="Numeri nel gutter">
        <Toggle on={tweaks.showChunkNumbers} onChange={v=>set('showChunkNumbers', v)}/>
      </Row>
      <Row label="Glow chunk attivo">
        <Toggle on={tweaks.glowActiveChunk} onChange={v=>set('glowActiveChunk', v)}/>
      </Row>

      <Row label="Stile linea">
        <SegControl
          options={[['none','—'],['curve','curva'],['line','retta'],['dashed','punt.']]}
          value={tweaks.linkStyle}
          onChange={v=>set('linkStyle', v)}
        />
      </Row>

      <Row label={`Tinta accento · ${tweaks.accentHue}°`}>
        <input type="range" min="200" max="310" value={tweaks.accentHue}
          onChange={e=>set('accentHue', Number(e.target.value))}
          style={{ width: 140 }}/>
      </Row>
    </div>
  );
}

function Row({ label, children }) {
  return (
    <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between',
      marginBottom: 12, gap: 10 }}>
      <div style={{ fontSize: 12, opacity: 0.85 }}>{label}</div>
      {children}
    </div>
  );
}

function Toggle({ on, onChange }) {
  return (
    <div onClick={()=>onChange(!on)} style={{
      width: 36, height: 20, borderRadius: 999, cursor:'pointer',
      background: on ? 'oklch(0.7 0.14 250)' : 'rgba(239,229,207,0.15)',
      position:'relative', transition:'all 0.15s',
    }}>
      <div style={{ position:'absolute', top:2, left: on?18:2,
        width:16, height:16, borderRadius:'50%', background:'#efe5cf',
        transition:'all 0.15s' }}/>
    </div>
  );
}

function SegControl({ options, value, onChange }) {
  return (
    <div style={{ display:'flex', padding:2, borderRadius:8,
      background:'rgba(239,229,207,0.06)',
    }}>
      {options.map(([k, l])=>(
        <div key={k} onClick={()=>onChange(k)} style={{
          padding:'4px 8px', borderRadius:6, fontSize:10, cursor:'pointer',
          background: value===k ? 'oklch(0.7 0.14 250)' : 'transparent',
          color: value===k ? '#0f0e10' : '#efe5cf',
          fontWeight: value===k ? 600 : 400,
        }}>{l}</div>
      ))}
    </div>
  );
}

function App() {
  return (
    <DesignCanvas>
      <div style={{ padding:'24px 60px 44px', maxWidth: 900 }}>
        <div style={{
          fontFamily:'Inter Tight, system-ui', fontSize:11, letterSpacing:2.5,
          color:'rgba(60,50,40,0.6)', textTransform:'uppercase', marginBottom: 14,
        }}>Marginalia · desktop · Inchiostro</div>
        <div style={{
          fontFamily:'Cormorant Garamond, serif', fontSize: 48, lineHeight: 1.05,
          color:'rgba(40,30,20,0.9)', letterSpacing:-0.8, fontWeight: 500,
          fontStyle:'italic', marginBottom: 14,
        }}>Lettura & impostazioni.</div>
        <div style={{
          fontFamily:'Cormorant Garamond, serif', fontStyle:'italic', fontSize:18,
          color:'rgba(60,50,40,0.7)', lineHeight:1.5, maxWidth: 720,
        }}>
          La finestra di lettura con le note ancorate al testo e la nuova pagina Impostazioni costruita sulla <em>SETTINGS_SPEC</em>: lingua e voce come controlli principali in cima, riconoscimento vocale con i due motori (Apple / Whisper) e le loro ragioni di disponibilità, tabella dei comandi vocali editabile, sezione installazioni come unica zona che tocca la rete, pulsante Applica che si illumina solo quando c'è qualcosa da salvare.
        </div>
      </div>

      <DCSection title="Lettura · con collegamenti visibili" subtitle="passa il mouse sulle note a destra — oppure attiva Tweaks per cambiare lo stile del link">
        <DCArtboard label="01 · Desktop — Inchiostro · lettura" width={1440} height={900}
          style={{ background:'transparent', boxShadow:'0 40px 100px rgba(0,0,0,0.25)', borderRadius: 14 }}>
          <Window/>
        </DCArtboard>
      </DCSection>

      <DCSection title="Impostazioni" subtitle="clicca l'icona a sinistra di Marginalia (in alto a sinistra) per aprire le impostazioni — sotto, già aperte; scorri il contenuto della finestra. tutte le modifiche vengono messe in sospeso finché non premi Applica.">
        <DCArtboard label="02 · Desktop — Impostazioni · lingua, voce, STT, comandi, installazioni" width={1440} height={900}
          style={{ background:'transparent', boxShadow:'0 40px 100px rgba(0,0,0,0.25)', borderRadius: 14 }}>
          <Window initialMode="settings"/>
        </DCArtboard>
      </DCSection>
    </DesignCanvas>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App/>);
