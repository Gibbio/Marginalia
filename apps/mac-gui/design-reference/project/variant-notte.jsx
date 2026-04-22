// Variant NOTTE — deepest ambient dark, Lora serif for reading,
// radial soft gradients, minimal chrome, almost no borders.
// Accent: ink blue, oklch(0.62 0.14 255)

const notte = {
  bg: '#0b0d12',               // near-black, cool
  bgSoft: '#12151c',
  text: '#e8e4d9',             // warm paper
  textDim: 'rgba(232,228,217,0.55)',
  textFaint: 'rgba(232,228,217,0.28)',
  accent: 'oklch(0.68 0.13 255)',        // ink blue, luminous
  accentDeep: 'oklch(0.48 0.14 255)',
  accentGlow: 'oklch(0.68 0.13 255 / 0.25)',
  serif: "'Lora', Georgia, serif",
  sans: "'Inter Tight', system-ui, sans-serif",
  mono: "'JetBrains Mono', monospace",
};

// Ambient glow behind content
function NotteAmbient({ top='20%', left='50%', size=500, color='oklch(0.55 0.12 255 / 0.28)' }) {
  return (
    <div style={{
      position: 'absolute', top, left, transform:'translate(-50%, -50%)',
      width: size, height: size, borderRadius:'50%',
      background: `radial-gradient(circle, ${color}, transparent 65%)`,
      filter: 'blur(24px)', pointerEvents: 'none', zIndex: 0,
    }}/>
  );
}

function NotteTabBar({ active = 'leggi' }) {
  const tabs = [
    { k:'libreria', label:'Libreria' },
    { k:'leggi', label:'Leggi' },
    { k:'note', label:'Note' },
  ];
  return (
    <div style={{
      position:'absolute', bottom:0, left:0, right:0, height:92,
      paddingBottom: 28, paddingTop: 8,
      display:'flex', alignItems:'center', justifyContent:'center', gap: 8,
      background: 'linear-gradient(to top, rgba(11,13,18,0.95), rgba(11,13,18,0))',
      zIndex: 40,
    }}>
      {tabs.map(t => (
        <div key={t.k} style={{
          padding: '10px 18px', borderRadius: 999,
          fontFamily: notte.sans, fontSize: 13, fontWeight: 500, letterSpacing: 0.2,
          color: active===t.k ? notte.text : notte.textFaint,
          background: active===t.k ? 'rgba(255,255,255,0.06)' : 'transparent',
          border: active===t.k ? '1px solid rgba(255,255,255,0.08)' : '1px solid transparent',
        }}>{t.label}</div>
      ))}
    </div>
  );
}

// ──────────────── Screen 1: LIBRERIA ────────────────
function NotteLibreria() {
  const items = [
    { title: 'La montagna incantata', sub: 'Thomas Mann · in ascolto', prog: 0.34, active: true },
    { title: 'Note al convegno di Rovereto', sub: 'bozza · 12 minuti', prog: 0.88 },
    { title: 'Appunti sul Simposio', sub: 'Platone · tradotto', prog: 0.12 },
    { title: 'Lettera a Giulia — v4', sub: 'bozza · 3 minuti', prog: 0.56 },
    { title: 'Il giovane Holden', sub: 'Salinger', prog: 0 },
  ];
  return (
    <MPhone bg={notte.bg} statusColor={notte.text}>
      <NotteAmbient top="14%" left="70%" size={420} color="oklch(0.55 0.12 255 / 0.22)" />
      <div style={{ position:'relative', zIndex:1, padding: '64px 28px 110px', height:'100%', overflow:'hidden' }}>
        <div style={{
          fontFamily: notte.sans, fontSize: 11, letterSpacing: 2,
          color: notte.textFaint, textTransform: 'uppercase', marginBottom: 18,
        }}>Marginalia</div>
        <div style={{
          fontFamily: notte.serif, fontSize: 34, lineHeight: 1.1, color: notte.text,
          fontWeight: 400, marginBottom: 6, letterSpacing: -0.3,
        }}>Buonasera, Giulia.</div>
        <div style={{
          fontFamily: notte.serif, fontStyle: 'italic', fontSize: 17,
          color: notte.textDim, marginBottom: 34, letterSpacing: 0.1,
        }}>Riprendiamo da capitolo tre?</div>

        <div style={{ display:'flex', flexDirection:'column', gap: 22 }}>
          {items.map((it, i) => (
            <div key={i} style={{ position: 'relative' }}>
              {it.active && (
                <div style={{
                  position:'absolute', left:-14, top:4, bottom:4, width:2,
                  background: notte.accent, borderRadius:2,
                  boxShadow: `0 0 12px ${notte.accentGlow}`,
                }}/>
              )}
              <div style={{
                fontFamily: notte.serif, fontSize: 19, color: notte.text,
                fontWeight: 400, lineHeight: 1.25, marginBottom: 4,
              }}>{it.title}</div>
              <div style={{
                fontFamily: notte.sans, fontSize: 12, color: notte.textDim,
                letterSpacing: 0.1, marginBottom: 10,
              }}>{it.sub}</div>
              {it.prog > 0 && (
                <div style={{ height:1, background:'rgba(232,228,217,0.1)', position:'relative' }}>
                  <div style={{
                    position:'absolute', top:0, left:0, height:1,
                    width: `${it.prog*100}%`,
                    background: it.active ? notte.accent : 'rgba(232,228,217,0.35)',
                  }}/>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
      <NotteTabBar active="libreria" />
    </MPhone>
  );
}

// ──────────────── Screen 2: LEGGI (document open / reading) ────────────────
function NotteLeggi() {
  // A passage of Mann-ish prose in Italian. One chunk is the current highlight.
  return (
    <MPhone bg={notte.bg} statusColor={notte.text}>
      <NotteAmbient top="42%" left="50%" size={480} color="oklch(0.58 0.13 255 / 0.20)" />
      {/* top meta */}
      <div style={{ position:'absolute', top:54, left:0, right:0, zIndex:5,
        display:'flex', alignItems:'center', justifyContent:'space-between',
        padding:'14px 24px',
      }}>
        <div style={{
          fontFamily: notte.sans, fontSize: 11, letterSpacing: 1.5,
          color: notte.textFaint, textTransform: 'uppercase',
        }}>Capitolo III · p. 47</div>
        <div style={{
          width: 28, height: 28, borderRadius: 999,
          border: `1px solid rgba(232,228,217,0.18)`,
          display:'flex', alignItems:'center', justifyContent:'center',
        }}>
          <div style={{ width:4, height:4, borderRadius:'50%', background: notte.accent,
            boxShadow: `0 0 8px ${notte.accent}`, animation: 'none' }}/>
        </div>
      </div>

      <div style={{ position:'relative', zIndex:1, padding: '120px 28px 200px',
        height: '100%', overflow: 'hidden',
        fontFamily: notte.serif, color: notte.textDim, fontSize: 19, lineHeight: 1.75,
      }}>
        <p style={{ margin: 0, marginBottom: 18 }}>
          <span style={{ color:'rgba(232,228,217,0.35)' }}>
            Il tempo, nell'alta montagna, non è il tempo della pianura. Si dilata, si contrae, talvolta sembra fermarsi del tutto, come se l'aria rarefatta ne modificasse la sostanza stessa.
          </span>
        </p>
        <p style={{ margin: 0, marginBottom: 18 }}>
          <span style={{
            color: notte.text,
            background: `linear-gradient(to right, oklch(0.58 0.13 255 / 0.18), oklch(0.58 0.13 255 / 0.08))`,
            boxShadow: `0 0 30px oklch(0.58 0.13 255 / 0.15), inset 0 0 0 1px oklch(0.68 0.13 255 / 0.2)`,
            padding: '2px 4px', borderRadius: 3,
          }}>
            Hans Castorp osservava la neve cadere oltre il vetro, e pensava — non senza un certo stupore — che erano passate già sette settimane dal suo arrivo,
          </span>
          <span style={{ color:'rgba(232,228,217,0.35)' }}> sette settimane che egli aveva contato come giorni, e che ora, al solo ricordarle, gli parevano un istante.</span>
        </p>
        <p style={{ margin: 0, color:'rgba(232,228,217,0.25)' }}>
          Ma forse, pensò, non è la durata a contare, quanto la qualità del tempo vissuto…
        </p>
      </div>

      {/* Margin tick for a note */}
      <div style={{ position:'absolute', right: 14, top: 320, zIndex: 3,
        display:'flex', alignItems:'center', gap: 6,
      }}>
        <div style={{ width:16, height:1, background: notte.accent, opacity:0.6 }}/>
        <div style={{ width:6, height:6, borderRadius:'50%', background: notte.accent,
          boxShadow:`0 0 10px ${notte.accent}` }}/>
      </div>

      {/* bottom player */}
      <div style={{
        position:'absolute', left:20, right:20, bottom: 108, zIndex: 20,
        padding:'14px 18px', borderRadius: 24,
        background:'rgba(18,21,28,0.7)',
        backdropFilter:'blur(16px)', WebkitBackdropFilter:'blur(16px)',
        border:'1px solid rgba(255,255,255,0.06)',
        display:'flex', alignItems:'center', gap: 14,
      }}>
        <div style={{ width:38, height:38, borderRadius:'50%',
          background: notte.accent, display:'flex', alignItems:'center', justifyContent:'center',
          boxShadow:`0 0 20px ${notte.accentGlow}`,
        }}>
          <div style={{ width:0, height:0,
            borderLeft:'8px solid #0b0d12', borderTop:'6px solid transparent',
            borderBottom:'6px solid transparent', marginLeft:3,
          }}/>
        </div>
        <div style={{ flex:1 }}>
          <div style={{ fontFamily: notte.sans, fontSize: 12, color: notte.text, fontWeight: 500 }}>
            La montagna incantata
          </div>
          <div style={{ fontFamily: notte.sans, fontSize: 10, color: notte.textFaint, letterSpacing:0.3, marginTop:2 }}>
            voce: elena · 1.0×
          </div>
        </div>
        {/* mini waveform */}
        <div style={{ display:'flex', alignItems:'center', gap:2, height:20 }}>
          {[4,9,6,14,7,11,5,8,12,6,3].map((h,i)=>(
            <div key={i} style={{
              width:2, height:h, background: i<5 ? notte.accent : 'rgba(232,228,217,0.25)',
              borderRadius:1,
            }}/>
          ))}
        </div>
      </div>

      <NotteTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 3: REGISTRANDO (voice note) ────────────────
function NotteRegistra() {
  return (
    <MPhone bg={notte.bg} statusColor={notte.text}>
      {/* Intense ambient — listening */}
      <NotteAmbient top="38%" left="50%" size={620} color="oklch(0.62 0.16 255 / 0.35)" />
      <NotteAmbient top="38%" left="50%" size={300} color="oklch(0.72 0.14 255 / 0.25)" />

      <div style={{ position:'absolute', top:74, left:0, right:0, zIndex:3,
        textAlign:'center',
      }}>
        <div style={{
          fontFamily: notte.sans, fontSize: 11, letterSpacing: 2.5,
          color: notte.textFaint, textTransform: 'uppercase', marginBottom: 14,
        }}>In ascolto</div>
        <div style={{
          fontFamily: notte.serif, fontSize: 15, fontStyle:'italic',
          color: notte.textDim, padding:'0 40px', lineHeight: 1.5,
        }}>
          «…erano passate già sette settimane dal suo arrivo…»
        </div>
      </div>

      {/* Breathing orb */}
      <div style={{ position:'absolute', top: '42%', left:'50%', transform:'translate(-50%, -50%)',
        zIndex: 4,
      }}>
        <div style={{
          width: 180, height: 180, borderRadius:'50%',
          background:'radial-gradient(circle at 40% 40%, oklch(0.78 0.12 255), oklch(0.48 0.15 255))',
          boxShadow:`0 0 80px oklch(0.65 0.16 255 / 0.5), inset -20px -20px 60px oklch(0.35 0.12 255 / 0.6)`,
          position:'relative',
        }}>
          {/* inner ring */}
          <div style={{
            position:'absolute', inset:-14, borderRadius:'50%',
            border:'1px solid oklch(0.72 0.14 255 / 0.35)',
          }}/>
          <div style={{
            position:'absolute', inset:-32, borderRadius:'50%',
            border:'1px solid oklch(0.72 0.14 255 / 0.18)',
          }}/>
          <div style={{
            position:'absolute', inset:-52, borderRadius:'50%',
            border:'1px solid oklch(0.72 0.14 255 / 0.08)',
          }}/>
        </div>
      </div>

      {/* Transcribed (live) text */}
      <div style={{ position:'absolute', bottom: 230, left: 28, right: 28, zIndex:3,
        textAlign:'center',
      }}>
        <div style={{
          fontFamily: notte.serif, fontSize: 21, lineHeight: 1.5, color: notte.text,
          fontStyle:'italic',
        }}>
          "Qui Mann usa <span style={{ color: notte.accent, fontStyle:'normal' }}>l'aggettivo</span> temporale come se fosse spazio…"
        </div>
        <div style={{
          marginTop: 14, display:'inline-flex', alignItems:'center', gap: 4,
        }}>
          {[1,2,3].map(i=>(
            <div key={i} style={{
              width:3, height:3, borderRadius:'50%', background: notte.textFaint,
            }}/>
          ))}
        </div>
      </div>

      {/* Action hint */}
      <div style={{ position:'absolute', bottom: 150, left: 0, right: 0, zIndex:3,
        textAlign:'center',
      }}>
        <div style={{
          display:'inline-flex', alignItems:'center', gap: 8,
          padding:'8px 14px', borderRadius: 999,
          background:'rgba(255,255,255,0.04)',
          border:'1px solid rgba(255,255,255,0.06)',
          fontFamily: notte.sans, fontSize: 11, color: notte.textDim,
          letterSpacing: 0.3,
        }}>
          <div style={{ width:5, height:5, borderRadius:'50%', background:'#e66', boxShadow:'0 0 6px #e66'}}/>
          di' "fatto" per salvare
        </div>
      </div>

      <NotteTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 4: RIELABORAZIONE (AI rewrite) ────────────────
function NotteRielabora() {
  return (
    <MPhone bg={notte.bg} statusColor={notte.text}>
      <NotteAmbient top="12%" left="30%" size={360} color="oklch(0.55 0.12 255 / 0.18)" />

      <div style={{ position:'absolute', top:54, left:0, right:0, zIndex:5,
        display:'flex', alignItems:'center', justifyContent:'space-between',
        padding:'14px 24px',
      }}>
        <div style={{
          fontFamily: notte.sans, fontSize: 11, letterSpacing: 1.5,
          color: notte.textFaint, textTransform: 'uppercase',
        }}>Rielaborazione</div>
        <div style={{
          fontFamily: notte.mono, fontSize: 10, color: notte.textFaint, letterSpacing: 0.5,
        }}>v2 · 1 nota</div>
      </div>

      <div style={{ position:'relative', zIndex:1, padding: '110px 24px 180px', height:'100%', overflow:'hidden' }}>

        {/* Original */}
        <div style={{ marginBottom: 22 }}>
          <div style={{
            fontFamily: notte.sans, fontSize: 10, letterSpacing: 2,
            color: notte.textFaint, textTransform: 'uppercase', marginBottom: 10,
          }}>Originale</div>
          <div style={{
            fontFamily: notte.serif, fontSize: 17, lineHeight: 1.6,
            color: notte.textDim, fontStyle:'italic',
            paddingLeft: 12,
            borderLeft: '1px solid rgba(232,228,217,0.15)',
          }}>
            Hans Castorp osservava la neve cadere oltre il vetro, e pensava che erano passate già sette settimane dal suo arrivo.
          </div>
        </div>

        {/* Note */}
        <div style={{
          marginBottom: 22, padding:'12px 14px', borderRadius: 10,
          background:'rgba(255,255,255,0.03)',
          border:'1px solid rgba(255,255,255,0.05)',
        }}>
          <div style={{ display:'flex', alignItems:'center', gap: 8, marginBottom: 6 }}>
            <div style={{ width:6, height:6, borderRadius:'50%', background: notte.accent }}/>
            <div style={{
              fontFamily: notte.sans, fontSize: 10, letterSpacing: 1.5,
              color: notte.textFaint, textTransform: 'uppercase',
            }}>La tua nota · 0:14</div>
          </div>
          <div style={{
            fontFamily: notte.serif, fontSize: 15, lineHeight: 1.5,
            color: notte.text, fontStyle:'italic',
          }}>
            "Qui il tempo è trattato come uno spazio che si può attraversare. Rendilo più sensoriale — fai sentire la lentezza."
          </div>
        </div>

        {/* Rewritten */}
        <div>
          <div style={{
            fontFamily: notte.sans, fontSize: 10, letterSpacing: 2,
            color: notte.accent, textTransform: 'uppercase', marginBottom: 10,
            display:'flex', alignItems:'center', gap: 8,
          }}>
            <div style={{ width:4, height:4, borderRadius:'50%', background: notte.accent,
              boxShadow:`0 0 8px ${notte.accent}` }}/>
            Riscritto
          </div>
          <div style={{
            fontFamily: notte.serif, fontSize: 17, lineHeight: 1.7,
            color: notte.text,
          }}>
            Hans Castorp guardava la neve che scendeva lenta oltre il vetro, fiocco dopo fiocco, e quelle sette settimane — attraversate giorno per giorno come un lungo corridoio di silenzio — gli parvero improvvisamente un solo, interminabile pomeriggio.
          </div>
        </div>
      </div>

      {/* bottom actions */}
      <div style={{
        position:'absolute', left:20, right:20, bottom: 108, zIndex: 20,
        display:'flex', gap: 8,
      }}>
        <div style={{
          flex: 1, padding:'14px 18px', borderRadius: 18,
          background:'rgba(18,21,28,0.7)',
          backdropFilter:'blur(16px)',
          border:'1px solid rgba(255,255,255,0.06)',
          fontFamily: notte.sans, fontSize: 13, color: notte.textDim,
          textAlign:'center', letterSpacing: 0.2,
        }}>Ignora</div>
        <div style={{
          flex: 1.3, padding:'14px 18px', borderRadius: 18,
          background: notte.accent, color:'#0b0d12',
          fontFamily: notte.sans, fontSize: 13, fontWeight: 500,
          textAlign:'center', letterSpacing: 0.2,
          boxShadow:`0 0 30px ${notte.accentGlow}`,
        }}>Ascolta riscritto</div>
      </div>

      <NotteTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 5: NOTE (list) ────────────────
function NotteNote() {
  const notes = [
    { time:'0:14', doc:'La montagna incantata · cap. III',
      text:'"Qui il tempo è trattato come uno spazio che si può attraversare."',
      duration:'14"', action:'rielaborato' },
    { time:'ieri · 18:22', doc:'Lettera a Giulia — v4',
      text:'"Togli il secondo paragrafo. Diventa più diretto quando ti fermi."',
      duration:'9"', action:'applicato' },
    { time:'ieri · 17:04', doc:'Appunti sul Simposio',
      text:'"Controlla la traduzione di εἰς — qui non è verso, è dentro."',
      duration:'21"', action:'in sospeso' },
    { time:'mar · 09:12', doc:'Note al convegno di Rovereto',
      text:'"Aggiungi la citazione di Calvino sulle città invisibili."',
      duration:'8"', action:'applicato' },
  ];
  return (
    <MPhone bg={notte.bg} statusColor={notte.text}>
      <NotteAmbient top="8%" left="80%" size={340} color="oklch(0.55 0.12 255 / 0.18)" />
      <div style={{ position:'relative', zIndex:1, padding: '64px 0 110px', height:'100%', overflow:'hidden' }}>
        <div style={{ padding:'0 28px 22px' }}>
          <div style={{
            fontFamily: notte.sans, fontSize: 11, letterSpacing: 2,
            color: notte.textFaint, textTransform: 'uppercase', marginBottom: 12,
          }}>Marginalia · 47</div>
          <div style={{
            fontFamily: notte.serif, fontSize: 30, lineHeight: 1.1, color: notte.text,
            fontWeight: 400, letterSpacing: -0.3,
          }}>Le tue note.</div>
        </div>

        <div style={{ display:'flex', flexDirection:'column' }}>
          {notes.map((n, i) => (
            <div key={i} style={{
              padding:'18px 28px',
              borderTop:'1px solid rgba(232,228,217,0.06)',
              position: 'relative',
            }}>
              <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between',
                marginBottom: 8,
              }}>
                <div style={{
                  fontFamily: notte.sans, fontSize: 11, color: notte.textFaint,
                  letterSpacing: 0.3,
                }}>{n.time}</div>
                <div style={{
                  fontFamily: notte.mono, fontSize: 9, letterSpacing: 0.5,
                  color: n.action==='applicato' ? notte.accent
                       : n.action==='rielaborato' ? notte.accent
                       : notte.textFaint,
                  textTransform:'uppercase',
                }}>{n.action}</div>
              </div>
              <div style={{
                fontFamily: notte.serif, fontSize: 16, lineHeight: 1.45, color: notte.text,
                fontStyle:'italic', marginBottom: 10,
              }}>{n.text}</div>
              <div style={{ display:'flex', alignItems:'center', gap: 10 }}>
                <div style={{ display:'flex', alignItems:'center', gap:1.5, height:12 }}>
                  {[3,6,4,8,5,7,4,6,9,5,3,5,7,4,2].map((h,j)=>(
                    <div key={j} style={{
                      width:1.5, height:h,
                      background: j<5 ? notte.accent : 'rgba(232,228,217,0.25)',
                      borderRadius:1,
                    }}/>
                  ))}
                </div>
                <div style={{
                  fontFamily: notte.mono, fontSize: 10, color: notte.textFaint,
                }}>{n.duration}</div>
                <div style={{ flex:1 }}/>
                <div style={{
                  fontFamily: notte.sans, fontSize: 11, color: notte.textDim,
                  maxWidth: 140, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap',
                }}>{n.doc}</div>
              </div>
            </div>
          ))}
        </div>
      </div>
      <NotteTabBar active="note" />
    </MPhone>
  );
}

// ──────────────── Screen 6: RIPRESA (resume / handoff) ────────────────
function NotteRipresa() {
  return (
    <MPhone bg={notte.bg} statusColor={notte.text}>
      <NotteAmbient top="42%" left="50%" size={540} color="oklch(0.55 0.12 255 / 0.26)" />

      <div style={{ position:'absolute', top: 120, left: 28, right: 28, zIndex: 2 }}>
        <div style={{
          fontFamily: notte.sans, fontSize: 11, letterSpacing: 2.5,
          color: notte.textFaint, textTransform: 'uppercase', marginBottom: 14,
        }}>Riprendi da</div>
        <div style={{
          fontFamily: notte.serif, fontSize: 28, lineHeight: 1.15,
          color: notte.text, letterSpacing: -0.3, marginBottom: 10,
        }}>La montagna<br/>incantata</div>
        <div style={{
          fontFamily: notte.serif, fontStyle:'italic', fontSize: 15,
          color: notte.textDim,
        }}>capitolo III · pagina 47</div>
      </div>

      {/* Quote card */}
      <div style={{ position:'absolute', top: 320, left: 28, right: 28, zIndex: 2,
        padding: '22px 22px', borderRadius: 16,
        background:'rgba(18,21,28,0.55)',
        backdropFilter:'blur(14px)', WebkitBackdropFilter:'blur(14px)',
        border:'1px solid rgba(255,255,255,0.06)',
      }}>
        <div style={{
          fontFamily: notte.serif, fontSize: 16, lineHeight: 1.55,
          color: notte.textDim, fontStyle:'italic',
        }}>
          «…<span style={{ color: notte.text, fontStyle:'normal' }}>erano passate già sette settimane</span> dal suo arrivo, sette settimane che egli aveva contato come giorni…»
        </div>
        <div style={{
          marginTop: 14, display:'flex', alignItems:'center', gap: 8,
          fontFamily: notte.sans, fontSize: 11, color: notte.textFaint, letterSpacing: 0.3,
        }}>
          <div style={{ width:1, height:10, background: notte.accent }}/>
          ultima frase ascoltata · 2 minuti fa
        </div>
      </div>

      {/* Big play CTA */}
      <div style={{ position:'absolute', bottom: 160, left: 0, right: 0, zIndex: 3,
        display:'flex', flexDirection:'column', alignItems:'center', gap: 14,
      }}>
        <div style={{
          width: 88, height: 88, borderRadius:'50%',
          background: notte.accent,
          boxShadow:`0 0 50px ${notte.accentGlow}, 0 0 0 1px oklch(0.8 0.1 255 / 0.3)`,
          display:'flex', alignItems:'center', justifyContent:'center',
        }}>
          <div style={{ width:0, height:0,
            borderLeft:'22px solid #0b0d12',
            borderTop:'15px solid transparent',
            borderBottom:'15px solid transparent',
            marginLeft: 6,
          }}/>
        </div>
        <div style={{
          fontFamily: notte.sans, fontSize: 12, color: notte.textDim,
          letterSpacing: 0.3,
        }}>o di' "riprendi"</div>
      </div>

      <NotteTabBar active="leggi" />
    </MPhone>
  );
}

Object.assign(window, {
  NotteLibreria, NotteLeggi, NotteRegistra, NotteRielabora, NotteNote, NotteRipresa,
});
