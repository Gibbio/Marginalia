// Variant INCHIOSTRO — the marginalia metaphor made literal.
// Dark vellum bg, warmer paper tones, Cormorant Garamond for reading,
// visible margin rule where notes live connected by thin ink lines.
// Accent: luminous ink blue.

const inch = {
  bg: '#0f0e10',                  // very dark warm
  paper: '#1a1815',              // "vellum"
  text: '#efe5cf',               // warm ivory
  textDim: 'rgba(239,229,207,0.55)',
  textFaint: 'rgba(239,229,207,0.25)',
  rule: 'rgba(239,229,207,0.1)',
  accent: 'oklch(0.7 0.14 250)',
  accentDeep: 'oklch(0.5 0.15 250)',
  accentSoft: 'oklch(0.7 0.14 250 / 0.18)',
  serif: "'Cormorant Garamond', Georgia, serif",
  sans: "'Inter Tight', system-ui, sans-serif",
  mono: "'JetBrains Mono', monospace",
};

function InchTabBar({ active = 'leggi' }) {
  const tabs = [
    { k:'libreria', label:'libreria' },
    { k:'leggi', label:'leggi' },
    { k:'note', label:'note' },
  ];
  return (
    <div style={{
      position:'absolute', bottom:0, left:0, right:0, height:92,
      paddingBottom: 30, paddingTop: 12,
      display:'flex', alignItems:'center', justifyContent:'center', gap: 28,
      background: 'linear-gradient(to top, rgba(15,14,16,0.98), rgba(15,14,16,0))',
      zIndex: 40,
    }}>
      {tabs.map(t => (
        <div key={t.k} style={{ position:'relative', padding:'4px 2px',
          fontFamily: inch.serif, fontStyle:'italic', fontSize: 17,
          color: active===t.k ? inch.text : inch.textFaint,
          letterSpacing: 0.3,
        }}>
          {t.label}
          {active===t.k && (
            <div style={{ position:'absolute', left:0, right:0, bottom:-4,
              height:1, background: inch.accent, boxShadow:`0 0 6px ${inch.accent}` }}/>
          )}
        </div>
      ))}
    </div>
  );
}

// ──────────────── Screen 1: LIBRERIA ────────────────
function InchLibreria() {
  const items = [
    { title:'La montagna incantata', author:'Thomas Mann', notes: 12, pct:34, active:true },
    { title:'Lettera a Giulia', author:'bozza · v4', notes: 8, pct:88 },
    { title:'Appunti sul Simposio', author:'Platone', notes: 4, pct:12 },
    { title:'Il giovane Holden', author:'J.D. Salinger', notes: 0, pct:0 },
  ];
  return (
    <MPhone bg={inch.bg} statusColor={inch.text}>
      <div style={{ position:'relative', zIndex:1, padding:'64px 0 110px', height:'100%', overflow:'hidden' }}>
        <div style={{ padding:'0 26px 26px' }}>
          <div style={{
            fontFamily: inch.mono, fontSize: 10, letterSpacing: 2,
            color: inch.textFaint, textTransform:'lowercase', marginBottom: 20,
          }}>marginalia — i tuoi margini</div>
          <div style={{
            fontFamily: inch.serif, fontSize: 40, lineHeight: 1,
            color: inch.text, fontWeight: 500, letterSpacing: -0.5, marginBottom:6,
          }}>Scaffale</div>
          <div style={{
            fontFamily: inch.serif, fontStyle:'italic', fontSize: 17, color: inch.textDim,
          }}>quattro letture in corso</div>
        </div>

        {items.map((it, i) => (
          <div key={i} style={{
            padding: '22px 26px', position:'relative',
            borderTop:'1px solid rgba(239,229,207,0.08)',
            borderBottom: i===items.length-1 ? '1px solid rgba(239,229,207,0.08)' : undefined,
            display:'flex', alignItems:'flex-start', gap: 16,
          }}>
            {/* book edge */}
            <div style={{ width:44, height: 64, position:'relative', flexShrink:0 }}>
              <MPlaceholder w={44} h={64} label="copertina" radius={2}
                tone="rgba(239,229,207,0.06)" stroke="rgba(239,229,207,0.1)"/>
              {it.pct>0 && (
                <div style={{ position:'absolute', left:0, bottom:-6, height:1,
                  width: `${it.pct}%`, background: it.active ? inch.accent : inch.textFaint,
                  boxShadow: it.active ? `0 0 4px ${inch.accent}` : 'none',
                }}/>
              )}
            </div>
            <div style={{ flex:1, minWidth:0 }}>
              <div style={{
                fontFamily: inch.serif, fontSize: 22, lineHeight: 1.15,
                color: inch.text, marginBottom: 4,
              }}>{it.title}</div>
              <div style={{
                fontFamily: inch.serif, fontStyle:'italic', fontSize: 14,
                color: inch.textDim, marginBottom: 10,
              }}>{it.author}</div>
              <div style={{ display:'flex', alignItems:'center', gap: 10 }}>
                <div style={{
                  fontFamily: inch.mono, fontSize: 10, color: inch.textFaint,
                }}>{it.pct}%</div>
                {it.notes>0 && (
                  <>
                    <div style={{ width:1, height:10, background: inch.rule }}/>
                    <div style={{ display:'flex', alignItems:'center', gap:4 }}>
                      <div style={{ width:4, height:4, borderRadius:'50%', background: inch.accent }}/>
                      <div style={{ fontFamily: inch.mono, fontSize: 10, color: inch.textDim }}>
                        {it.notes} note
                      </div>
                    </div>
                  </>
                )}
              </div>
            </div>
            {it.active && (
              <div style={{
                fontFamily: inch.serif, fontStyle:'italic', fontSize: 12,
                color: inch.accent, alignSelf:'flex-start', marginTop: 4,
              }}>in ascolto</div>
            )}
          </div>
        ))}
      </div>
      <InchTabBar active="libreria" />
    </MPhone>
  );
}

// ──────────────── Screen 2: LEGGI ────────────────
function InchLeggi() {
  // layout: text column with visible margin rule at right; notes sit in the margin,
  // connected to the current chunk by thin lines.
  const marginX = 70; // right margin reserved for notes
  return (
    <MPhone bg={inch.bg} statusColor={inch.text}>
      {/* top meta */}
      <div style={{ position:'absolute', top:54, left:0, right:0, zIndex:5,
        display:'flex', alignItems:'center', justifyContent:'space-between',
        padding:'14px 22px',
      }}>
        <div style={{
          fontFamily: inch.mono, fontSize: 10, letterSpacing: 1.5,
          color: inch.textFaint,
        }}>cap. III — p. 47</div>
        <div style={{ display:'flex', gap:8, alignItems:'center' }}>
          <div style={{ width:4, height:4, borderRadius:'50%',
            background: inch.accent, boxShadow:`0 0 8px ${inch.accent}` }}/>
          <div style={{
            fontFamily: inch.serif, fontStyle:'italic', fontSize: 13,
            color: inch.textDim,
          }}>in ascolto</div>
        </div>
      </div>

      {/* reading column */}
      <div style={{ position:'absolute', top: 110, left: 22, right: marginX, bottom: 108,
        zIndex: 1, overflow:'hidden',
        fontFamily: inch.serif, color: inch.textDim, fontSize: 20, lineHeight: 1.55,
      }}>
        <p style={{ margin: 0, marginBottom: 14, color:'rgba(239,229,207,0.3)' }}>
          Il tempo, nell'alta montagna, non è il tempo della pianura. Si dilata, si contrae, talvolta sembra fermarsi del tutto.
        </p>
        <p style={{ margin: 0, marginBottom: 14 }}>
          <span style={{ color: inch.text,
            background: `linear-gradient(to bottom, transparent 55%, ${inch.accentSoft} 55%, ${inch.accentSoft} 92%, transparent 92%)`,
          }}>
            Hans Castorp osservava la neve cadere oltre il vetro, e pensava che erano passate già sette settimane dal suo arrivo,
          </span>
          <span style={{ color:'rgba(239,229,207,0.35)' }}> sette settimane che egli aveva contato come giorni.</span>
        </p>
        <p style={{ margin: 0, color:'rgba(239,229,207,0.22)' }}>
          Ma forse, pensò, non è la durata a contare…
        </p>
      </div>

      {/* margin rule */}
      <div style={{ position:'absolute', top:110, bottom:108, right: marginX - 6,
        width: 1, background: inch.rule, zIndex: 2,
      }}/>

      {/* marginal notes */}
      <Note y={225} label="t.c." text="tempo = spazio" active />
      <Note y={295} label="$" text="v. Proust, Recherche" />
      <Note y={362} label="?" text="tradurre 'contato'?" />

      {/* bottom player */}
      <div style={{
        position:'absolute', left:18, right:18, bottom: 108, zIndex: 20,
        padding:'12px 14px', borderRadius: 14,
        background:'rgba(26,24,21,0.7)',
        backdropFilter:'blur(14px)', WebkitBackdropFilter:'blur(14px)',
        border:'1px solid rgba(239,229,207,0.08)',
        display:'flex', alignItems:'center', gap: 12,
      }}>
        <div style={{ width:34, height:34, borderRadius:'50%',
          border:`1px solid ${inch.accent}`,
          display:'flex', alignItems:'center', justifyContent:'center',
          boxShadow:`inset 0 0 12px ${inch.accentSoft}`,
        }}>
          <div style={{ display:'flex', gap:2.5 }}>
            <div style={{ width:3, height:11, background: inch.accent }}/>
            <div style={{ width:3, height:11, background: inch.accent }}/>
          </div>
        </div>
        <div style={{ flex:1 }}>
          <div style={{ fontFamily: inch.serif, fontSize: 14, color: inch.text, fontStyle:'italic' }}>
            "…sette settimane dal suo arrivo…"
          </div>
          <div style={{ fontFamily: inch.mono, fontSize: 9, color: inch.textFaint, letterSpacing:0.3, marginTop:2 }}>
            00:47 / 12:03 · 1.0×
          </div>
        </div>
      </div>

      <InchTabBar active="leggi" />
    </MPhone>
  );

  function Note({ y, label, text, active }) {
    return (
      <>
        {/* tick line from margin rule into the margin */}
        <div style={{
          position:'absolute', top: y+8, right: marginX - 6, width: 10, height: 1,
          background: active ? inch.accent : inch.rule, zIndex: 3,
        }}/>
        <div style={{
          position:'absolute', top: y, right: 10, width: marginX - 20, zIndex: 3,
        }}>
          <div style={{
            fontFamily: inch.serif, fontStyle:'italic', fontSize: 11,
            color: active ? inch.accent : inch.textFaint,
            marginBottom: 2, letterSpacing: 0.3,
          }}>{label}</div>
          <div style={{
            fontFamily: inch.serif, fontSize: 12, lineHeight: 1.2,
            color: active ? inch.text : inch.textDim,
          }}>{text}</div>
        </div>
      </>
    );
  }
}

// ──────────────── Screen 3: REGISTRANDO ────────────────
function InchRegistra() {
  return (
    <MPhone bg={inch.bg} statusColor={inch.text}>
      <div style={{ position:'absolute', inset:0, background:
        'radial-gradient(ellipse at 50% 45%, oklch(0.35 0.13 250 / 0.28), transparent 60%)',
        zIndex:0,
      }}/>

      {/* Context quote — dimmed */}
      <div style={{ position:'absolute', top: 90, left: 26, right: 26, zIndex: 2 }}>
        <div style={{
          fontFamily: inch.mono, fontSize: 9, letterSpacing: 2,
          color: inch.textFaint, marginBottom: 10,
        }}>sull'ultima frase</div>
        <div style={{
          fontFamily: inch.serif, fontSize: 17, lineHeight: 1.5,
          color: inch.textDim, fontStyle:'italic',
          paddingLeft: 12, borderLeft: `1px solid ${inch.rule}`,
        }}>
          Hans Castorp osservava la neve cadere oltre il vetro, e pensava che erano passate già sette settimane dal suo arrivo.
        </div>
      </div>

      {/* Live transcript — large serif, handwritten feel via italic */}
      <div style={{ position:'absolute', top: 280, left: 26, right: 26, zIndex: 2 }}>
        <div style={{
          fontFamily: inch.mono, fontSize: 9, letterSpacing: 2,
          color: inch.accent, marginBottom: 14,
          display:'flex', alignItems:'center', gap: 8,
        }}>
          <div style={{ width:5, height:5, borderRadius:'50%', background:'#e66',
            boxShadow:'0 0 6px #e66' }}/>
          stai parlando
        </div>
        <div style={{
          fontFamily: inch.serif, fontSize: 23, lineHeight: 1.45,
          color: inch.text, fontStyle:'italic',
        }}>
          Qui il tempo non scorre, <span style={{ color: inch.accent }}>si accumula</span>
          <span style={{ color: inch.textDim }}> — come la neve sul davanzale</span>
          <span style={{ display:'inline-block', width:2, height: 22, background: inch.accent,
            marginLeft: 3, verticalAlign:'middle',
            boxShadow:`0 0 6px ${inch.accent}` }}/>
        </div>
      </div>

      {/* Waveform */}
      <div style={{ position:'absolute', left: 26, right: 26, bottom: 180, zIndex: 2,
        display:'flex', alignItems:'center', justifyContent:'center', gap: 2, height: 40,
      }}>
        {Array.from({length: 48}).map((_,i)=>{
          const h = 4 + Math.abs(Math.sin(i*0.6)) * (i<30 ? 28 : 8) + (i%3)*3;
          return (
            <div key={i} style={{
              width:2, height:h, borderRadius:1,
              background: i<30 ? inch.accent : 'rgba(239,229,207,0.2)',
              opacity: i<30 ? 0.4 + Math.sin(i*0.3)*0.5 : 0.35,
            }}/>
          );
        })}
      </div>

      {/* Hint */}
      <div style={{ position:'absolute', bottom: 130, left: 0, right: 0, zIndex:3,
        textAlign:'center',
      }}>
        <div style={{
          fontFamily: inch.serif, fontStyle:'italic', fontSize: 14,
          color: inch.textFaint,
        }}>di' <span style={{ color: inch.textDim }}>"fatto"</span> per salvare, <span style={{ color: inch.textDim }}>"rielabora"</span> per riscrivere</div>
      </div>

      <InchTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 4: RIELABORAZIONE ────────────────
function InchRielabora() {
  return (
    <MPhone bg={inch.bg} statusColor={inch.text}>
      <div style={{ position:'absolute', top:54, left:0, right:0, zIndex:5,
        display:'flex', alignItems:'center', justifyContent:'space-between',
        padding:'14px 22px',
      }}>
        <div style={{
          fontFamily: inch.mono, fontSize: 10, letterSpacing: 1.5,
          color: inch.textFaint,
        }}>rielaborazione</div>
        <div style={{
          fontFamily: inch.serif, fontStyle:'italic', fontSize: 13,
          color: inch.textDim,
        }}>due versioni</div>
      </div>

      <div style={{ position:'relative', zIndex:1, padding:'108px 22px 180px',
        height:'100%', overflow:'hidden',
      }}>
        {/* Original */}
        <div style={{ marginBottom: 20 }}>
          <div style={{
            fontFamily: inch.mono, fontSize: 9, letterSpacing: 2,
            color: inch.textFaint, marginBottom: 10,
          }}>originale</div>
          <div style={{
            fontFamily: inch.serif, fontSize: 17, lineHeight: 1.55,
            color: inch.textDim,
            padding: '0 0 0 12px', borderLeft: `1px solid ${inch.rule}`,
          }}>
            Hans Castorp osservava la neve cadere oltre il vetro, e pensava che erano passate già sette settimane dal suo arrivo.
          </div>
        </div>

        {/* The note — inline, italic, like a real marginal note */}
        <div style={{
          marginBottom: 20, display:'flex', gap: 12, alignItems:'flex-start',
          padding:'2px 0',
        }}>
          <div style={{
            fontFamily: inch.serif, fontStyle:'italic', fontSize: 26, lineHeight:1,
            color: inch.accent, flexShrink: 0,
          }}>❝</div>
          <div>
            <div style={{
              fontFamily: inch.serif, fontStyle:'italic', fontSize: 15,
              lineHeight: 1.5, color: inch.text,
            }}>
              Qui il tempo non scorre, si accumula — come la neve sul davanzale.
            </div>
            <div style={{
              fontFamily: inch.mono, fontSize: 9, letterSpacing: 1.5,
              color: inch.textFaint, marginTop: 6,
            }}>nota tua · 0:18</div>
          </div>
        </div>

        {/* Rewritten */}
        <div>
          <div style={{
            fontFamily: inch.mono, fontSize: 9, letterSpacing: 2,
            color: inch.accent, marginBottom: 10, display:'flex', alignItems:'center', gap:6,
          }}>
            <div style={{ width:4, height:4, borderRadius:'50%', background: inch.accent,
              boxShadow:`0 0 6px ${inch.accent}` }}/>
            riscritto · su tua nota
          </div>
          <div style={{
            fontFamily: inch.serif, fontSize: 17, lineHeight: 1.65,
            color: inch.text,
            padding: '0 0 0 12px', borderLeft: `1px solid ${inch.accent}`,
          }}>
            Hans Castorp guardava la neve deporsi lenta oltre il vetro, e sentiva che quelle sette settimane non erano passate via di lui: gli si erano posate addosso, strato su strato, come i fiocchi sul davanzale.
          </div>
        </div>
      </div>

      {/* actions */}
      <div style={{
        position:'absolute', left:22, right:22, bottom: 108, zIndex:20,
        display:'flex', gap: 10,
      }}>
        <div style={{
          flex: 1, padding:'13px 0', borderRadius: 0,
          fontFamily: inch.serif, fontStyle:'italic', fontSize: 14,
          color: inch.textDim, textAlign:'center',
          borderBottom:`1px solid ${inch.rule}`,
        }}>scarta</div>
        <div style={{
          flex: 1, padding:'13px 0',
          fontFamily: inch.serif, fontStyle:'italic', fontSize: 14,
          color: inch.textDim, textAlign:'center',
          borderBottom:`1px solid ${inch.rule}`,
        }}>tieni entrambe</div>
        <div style={{
          flex: 1.2, padding:'13px 0',
          fontFamily: inch.serif, fontStyle:'italic', fontSize: 14,
          color: inch.accent, textAlign:'center',
          borderBottom:`1px solid ${inch.accent}`,
          textShadow:`0 0 8px ${inch.accentSoft}`,
        }}>ascolta ↗</div>
      </div>

      <InchTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 5: NOTE ────────────────
function InchNote() {
  const groups = [
    { doc: 'La montagna incantata', notes: [
      { ts:'p. 47', text:'Qui il tempo non scorre, si accumula — come la neve.', dur:'18"', act:'riscritto' },
      { ts:'p. 42', text:'«sanatorio» qui è metafora o luogo reale?', dur:'07"', act:'' },
      { ts:'p. 38', text:'Confronta con l\u2019Ulisse — stream of consciousness.', dur:'24"', act:'' },
    ]},
    { doc: 'Lettera a Giulia — v4', notes: [
      { ts:'§ 2', text:'Togli il secondo paragrafo. Più diretta.', dur:'09"', act:'applicato' },
    ]},
    { doc: 'Appunti sul Simposio', notes: [
      { ts:'203a', text:'Controlla la traduzione di εἰς — qui non è verso, è dentro.', dur:'21"', act:'' },
    ]},
  ];
  return (
    <MPhone bg={inch.bg} statusColor={inch.text}>
      <div style={{ position:'relative', zIndex:1, padding:'64px 0 110px', height:'100%', overflow:'hidden' }}>
        <div style={{ padding:'0 26px 24px' }}>
          <div style={{
            fontFamily: inch.mono, fontSize: 10, letterSpacing: 2,
            color: inch.textFaint, marginBottom: 18,
          }}>47 note · 3 documenti</div>
          <div style={{
            fontFamily: inch.serif, fontSize: 40, lineHeight: 1,
            color: inch.text, fontWeight: 500, letterSpacing: -0.5,
          }}>Margini</div>
        </div>

        {groups.map((g, gi) => (
          <div key={gi} style={{ marginBottom: 22 }}>
            <div style={{
              padding:'8px 26px',
              fontFamily: inch.serif, fontStyle:'italic', fontSize: 14,
              color: inch.accent,
              borderTop:'1px solid rgba(239,229,207,0.08)',
              borderBottom:'1px solid rgba(239,229,207,0.08)',
              background:'rgba(239,229,207,0.02)',
            }}>{g.doc}</div>
            {g.notes.map((n, ni) => (
              <div key={ni} style={{
                padding:'14px 26px',
                borderBottom: ni<g.notes.length-1 ? '1px solid rgba(239,229,207,0.05)' : undefined,
                display:'flex', gap: 14, alignItems:'flex-start',
              }}>
                <div style={{
                  fontFamily: inch.mono, fontSize: 10, color: inch.textFaint,
                  width: 36, flexShrink: 0, paddingTop: 3,
                }}>{n.ts}</div>
                <div style={{ flex:1, minWidth:0 }}>
                  <div style={{
                    fontFamily: inch.serif, fontStyle:'italic', fontSize: 15,
                    lineHeight: 1.4, color: inch.text, marginBottom: 6,
                  }}>{n.text}</div>
                  <div style={{ display:'flex', alignItems:'center', gap: 10 }}>
                    <div style={{ display:'flex', gap:1.5, height: 9 }}>
                      {[3,5,3,7,4,6,3,5,8,4,3,5,6,3].map((h,j)=>(
                        <div key={j} style={{ width:1.5, height:h, background: inch.accent,
                          opacity: 0.3+(j%3)*0.2 }}/>
                      ))}
                    </div>
                    <div style={{ fontFamily: inch.mono, fontSize: 9, color: inch.textFaint }}>{n.dur}</div>
                    {n.act && (
                      <>
                        <div style={{ width:1, height:8, background: inch.rule }}/>
                        <div style={{
                          fontFamily: inch.serif, fontStyle:'italic', fontSize: 11,
                          color: n.act==='applicato' ? inch.accent : inch.textDim,
                        }}>{n.act}</div>
                      </>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>
      <InchTabBar active="note" />
    </MPhone>
  );
}

// ──────────────── Screen 6: RIPRESA ────────────────
function InchRipresa() {
  return (
    <MPhone bg={inch.bg} statusColor={inch.text}>
      <div style={{ position:'absolute', inset:0, background:
        'radial-gradient(ellipse at 50% 60%, oklch(0.3 0.14 250 / 0.25), transparent 55%)',
        zIndex:0,
      }}/>

      <div style={{ position:'absolute', top:110, left: 26, right: 26, zIndex: 2 }}>
        <div style={{
          fontFamily: inch.mono, fontSize: 10, letterSpacing: 2,
          color: inch.textFaint, marginBottom: 14,
        }}>dove eri rimasto</div>
        <div style={{
          fontFamily: inch.serif, fontSize: 36, lineHeight: 1.05,
          color: inch.text, letterSpacing: -0.5, marginBottom: 6,
          fontStyle:'italic',
        }}>La montagna<br/>incantata</div>
        <div style={{
          fontFamily: inch.serif, fontSize: 14, color: inch.textDim,
        }}>Thomas Mann · capitolo III · p. 47</div>
      </div>

      {/* Page-like card with last heard sentence */}
      <div style={{ position:'absolute', top: 280, left: 26, right: 26, zIndex: 2,
        background: inch.paper,
        padding: '22px 20px', borderRadius: 2,
        boxShadow: '0 20px 40px rgba(0,0,0,0.4), inset 0 1px 0 rgba(255,255,255,0.03)',
        border:'1px solid rgba(239,229,207,0.06)',
      }}>
        <div style={{
          fontFamily: inch.mono, fontSize: 9, letterSpacing: 2,
          color: inch.textFaint, marginBottom: 10,
        }}>ultima frase · 2 minuti fa</div>
        <div style={{
          fontFamily: inch.serif, fontSize: 17, lineHeight: 1.55,
          color: inch.textDim, fontStyle:'italic',
        }}>
          «…<span style={{ color: inch.text, fontStyle:'normal',
            borderBottom:`1px solid ${inch.accent}`,
          }}>erano passate già sette settimane</span> dal suo arrivo, sette settimane che egli aveva contato come giorni.»
        </div>
        {/* 3 most recent notes inline */}
        <div style={{ marginTop: 18, paddingTop: 14, borderTop:`1px solid ${inch.rule}`,
          display:'flex', flexDirection:'column', gap: 6,
        }}>
          <div style={{ fontFamily: inch.mono, fontSize: 9, color: inch.textFaint, letterSpacing:1.5 }}>
            3 note in questo capitolo
          </div>
          <div style={{ display:'flex', gap: 10 }}>
            {['t.c.','$','?'].map((l,i)=>(
              <div key={i} style={{
                fontFamily: inch.serif, fontStyle:'italic', fontSize: 13,
                color: i===0 ? inch.accent : inch.textDim,
              }}>{l}</div>
            ))}
          </div>
        </div>
      </div>

      {/* CTA — a typographic button, not a circle */}
      <div style={{ position:'absolute', bottom: 150, left: 26, right: 26, zIndex: 3,
        padding:'18px 20px', borderRadius: 2,
        border:`1px solid ${inch.accent}`,
        background:'rgba(30,40,60,0.35)',
        backdropFilter:'blur(10px)',
        display:'flex', alignItems:'center', justifyContent:'space-between',
        boxShadow:`0 0 30px ${inch.accentSoft}`,
      }}>
        <div>
          <div style={{
            fontFamily: inch.serif, fontStyle:'italic', fontSize: 20,
            color: inch.text,
          }}>Riprendi la lettura</div>
          <div style={{
            fontFamily: inch.mono, fontSize: 9, letterSpacing: 1.5,
            color: inch.textFaint, marginTop: 2,
          }}>o di' "riprendi"</div>
        </div>
        <div style={{
          width: 44, height: 44, borderRadius:'50%',
          border:`1px solid ${inch.accent}`,
          display:'flex', alignItems:'center', justifyContent:'center',
          boxShadow:`inset 0 0 20px ${inch.accentSoft}`,
        }}>
          <div style={{ width:0, height:0,
            borderLeft:`10px solid ${inch.accent}`,
            borderTop:'7px solid transparent',
            borderBottom:'7px solid transparent',
            marginLeft: 3,
          }}/>
        </div>
      </div>

      <InchTabBar active="leggi" />
    </MPhone>
  );
}

Object.assign(window, {
  InchLibreria, InchLeggi, InchRegistra, InchRielabora, InchNote, InchRipresa,
});
