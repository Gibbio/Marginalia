// Variant ECO — audio-as-living-presence. Newsreader serif,
// most tactile audio visualization, layered concentric waves.
// Accent: violet-leaning ink blue oklch(0.65 0.14 270)

const eco = {
  bg: '#0a0a14',
  bgSoft: '#141423',
  text: '#ede7dc',
  textDim: 'rgba(237,231,220,0.58)',
  textFaint: 'rgba(237,231,220,0.28)',
  accent: 'oklch(0.72 0.13 270)',
  accentDeep: 'oklch(0.5 0.14 270)',
  accentSoft: 'oklch(0.72 0.13 270 / 0.2)',
  accentGlow: 'oklch(0.72 0.13 270 / 0.35)',
  serif: "'Newsreader', Georgia, serif",
  sans: "'Inter Tight', system-ui, sans-serif",
  mono: "'JetBrains Mono', monospace",
};

function EcoTabBar({ active = 'leggi' }) {
  const tabs = [
    { k:'libreria', label:'Libreria' },
    { k:'leggi', label:'Leggi' },
    { k:'note', label:'Note' },
  ];
  return (
    <div style={{
      position:'absolute', bottom:16, left: 20, right: 20, height: 58,
      borderRadius: 999,
      background:'rgba(20,20,35,0.7)',
      backdropFilter:'blur(18px)', WebkitBackdropFilter:'blur(18px)',
      border:'1px solid rgba(237,231,220,0.08)',
      display:'flex', alignItems:'center', justifyContent:'space-around',
      zIndex: 40, padding: '0 8px',
    }}>
      {tabs.map(t => (
        <div key={t.k} style={{
          position:'relative', padding:'10px 18px', borderRadius: 999,
          fontFamily: eco.sans, fontSize: 12, fontWeight: 500,
          color: active===t.k ? '#0a0a14' : eco.textDim,
          background: active===t.k ? eco.accent : 'transparent',
          boxShadow: active===t.k ? `0 0 20px ${eco.accentGlow}` : 'none',
          letterSpacing: 0.2,
        }}>{t.label}</div>
      ))}
    </div>
  );
}

// Concentric ring visualization (shared)
function EcoRings({ size = 260, intensity = 1, stroke = eco.accent }) {
  const rings = [1, 0.82, 0.66, 0.5, 0.38, 0.26];
  return (
    <div style={{ position:'relative', width: size, height: size }}>
      {rings.map((r, i) => (
        <div key={i} style={{
          position:'absolute', top:'50%', left:'50%',
          width: size*r, height: size*r, borderRadius:'50%',
          transform:'translate(-50%, -50%)',
          border: `1px solid ${stroke}`,
          opacity: (0.08 + (1-r)*0.22) * intensity,
        }}/>
      ))}
      {/* core */}
      <div style={{
        position:'absolute', top:'50%', left:'50%',
        width: size*0.16, height: size*0.16, borderRadius:'50%',
        transform:'translate(-50%, -50%)',
        background: `radial-gradient(circle, ${stroke}, transparent 70%)`,
        filter: 'blur(2px)',
        opacity: 0.6 * intensity,
      }}/>
    </div>
  );
}

// ──────────────── Screen 1: LIBRERIA ────────────────
function EcoLibreria() {
  const items = [
    { t:'La montagna incantata', a:'Thomas Mann', len:'11h 42m', pct:0.34, active:true },
    { t:'Il giovane Holden', a:'J.D. Salinger', len:'5h 18m', pct:0 },
    { t:'Appunti sul Simposio', a:'Platone', len:'2h 04m', pct:0.12 },
    { t:'Lettera a Giulia — v4', a:'tua bozza', len:'3m', pct:0.56 },
  ];
  return (
    <MPhone bg={eco.bg} statusColor={eco.text}>
      <div style={{ position:'absolute', top:-40, right:-80, zIndex:0, opacity:0.5 }}>
        <EcoRings size={340} />
      </div>

      <div style={{ position:'relative', zIndex:1, padding:'68px 24px 100px', height:'100%', overflow:'hidden' }}>
        <div style={{
          fontFamily: eco.sans, fontSize: 11, letterSpacing: 2.5,
          color: eco.textFaint, textTransform:'uppercase', marginBottom: 14,
        }}>In ascolto — oggi</div>

        {/* Hero — currently listening */}
        <div style={{
          padding: '22px 20px', borderRadius: 20,
          background:'linear-gradient(150deg, rgba(70,50,140,0.28), rgba(20,20,35,0.5))',
          border:'1px solid rgba(237,231,220,0.08)',
          marginBottom: 26, position:'relative', overflow:'hidden',
        }}>
          {/* mini rings */}
          <div style={{ position:'absolute', right:-30, top:-30, opacity:0.6 }}>
            <EcoRings size={160} />
          </div>
          <div style={{ position:'relative', zIndex:1 }}>
            <div style={{
              fontFamily: eco.serif, fontSize: 24, lineHeight: 1.1,
              color: eco.text, letterSpacing: -0.3, marginBottom: 4,
            }}>La montagna incantata</div>
            <div style={{
              fontFamily: eco.serif, fontStyle:'italic', fontSize: 14,
              color: eco.textDim, marginBottom: 18,
            }}>Thomas Mann · capitolo III</div>
            <div style={{ display:'flex', alignItems:'center', gap: 10 }}>
              <div style={{
                width: 36, height: 36, borderRadius:'50%',
                background: eco.accent,
                boxShadow:`0 0 20px ${eco.accentGlow}`,
                display:'flex', alignItems:'center', justifyContent:'center',
              }}>
                <div style={{ width:0, height:0,
                  borderLeft:'9px solid #0a0a14',
                  borderTop:'6px solid transparent',
                  borderBottom:'6px solid transparent', marginLeft: 3,
                }}/>
              </div>
              <div style={{ flex:1 }}>
                <div style={{ height:2, background:'rgba(237,231,220,0.1)', borderRadius:2,
                  position:'relative', overflow:'hidden',
                }}>
                  <div style={{
                    position:'absolute', left:0, top:0, height:2, width:'34%',
                    background: eco.accent, boxShadow:`0 0 6px ${eco.accent}`,
                    borderRadius: 2,
                  }}/>
                </div>
                <div style={{ display:'flex', justifyContent:'space-between', marginTop: 6 }}>
                  <div style={{ fontFamily: eco.mono, fontSize: 10, color: eco.textFaint }}>3h 58m</div>
                  <div style={{ fontFamily: eco.mono, fontSize: 10, color: eco.textFaint }}>11h 42m</div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div style={{
          fontFamily: eco.sans, fontSize: 11, letterSpacing: 2.5,
          color: eco.textFaint, textTransform:'uppercase', marginBottom: 14,
        }}>La tua libreria</div>

        <div style={{ display:'flex', flexDirection:'column', gap: 16 }}>
          {items.slice(1).map((it, i) => (
            <div key={i} style={{ display:'flex', alignItems:'center', gap: 14 }}>
              <MPlaceholder w={46} h={46} label="" radius={10}
                tone="rgba(237,231,220,0.05)" stroke="rgba(237,231,220,0.12)"/>
              <div style={{ flex:1, minWidth:0 }}>
                <div style={{
                  fontFamily: eco.serif, fontSize: 16, color: eco.text,
                  marginBottom: 2,
                }}>{it.t}</div>
                <div style={{
                  fontFamily: eco.sans, fontSize: 11, color: eco.textFaint,
                  letterSpacing: 0.2,
                }}>{it.a} · {it.len}</div>
              </div>
              {it.pct>0 && (
                <div style={{ fontFamily: eco.mono, fontSize: 10, color: eco.textDim }}>
                  {Math.round(it.pct*100)}%
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
      <EcoTabBar active="libreria" />
    </MPhone>
  );
}

// ──────────────── Screen 2: LEGGI ────────────────
function EcoLeggi() {
  return (
    <MPhone bg={eco.bg} statusColor={eco.text}>
      <div style={{ position:'absolute', top: -120, left:'50%', transform:'translateX(-50%)', zIndex:0, opacity:0.4 }}>
        <EcoRings size={460} />
      </div>

      <div style={{ position:'absolute', top:54, left:0, right:0, zIndex:5,
        display:'flex', alignItems:'center', justifyContent:'space-between',
        padding:'14px 22px',
      }}>
        <div style={{
          fontFamily: eco.sans, fontSize: 10, letterSpacing: 2,
          color: eco.textFaint, textTransform:'uppercase',
        }}>Cap. III · p. 47</div>
        <div style={{
          fontFamily: eco.mono, fontSize: 10, color: eco.accent,
          letterSpacing: 0.5,
        }}>04:23 / 12:03</div>
      </div>

      <div style={{ position:'relative', zIndex:1, padding:'100px 24px 180px',
        height:'100%', overflow:'hidden',
        fontFamily: eco.serif, color: eco.textDim, fontSize: 19, lineHeight: 1.7,
      }}>
        <p style={{ margin: 0, marginBottom: 16, color: 'rgba(237,231,220,0.3)' }}>
          Il tempo, nell'alta montagna, non è il tempo della pianura. Si dilata, si contrae.
        </p>
        <p style={{ margin: 0, marginBottom: 16 }}>
          <span style={{
            color: eco.text, fontWeight: 500,
            textShadow: `0 0 30px ${eco.accentGlow}`,
            background: `linear-gradient(180deg, transparent 0%, ${eco.accentSoft} 100%)`,
            padding: '1px 2px', borderRadius: 2,
          }}>
            Hans Castorp osservava la neve cadere oltre il vetro, e pensava che erano passate già sette settimane dal suo arrivo,
          </span>
          <span style={{ color: 'rgba(237,231,220,0.35)' }}> sette settimane che egli aveva contato come giorni.</span>
        </p>
        <p style={{ margin: 0, color: 'rgba(237,231,220,0.22)' }}>
          Ma forse, pensò, non è la durata a contare…
        </p>
      </div>

      {/* Audio wave as bottom panel — primary UI element */}
      <div style={{
        position:'absolute', left:20, right:20, bottom: 96, zIndex: 20,
        padding:'18px 18px', borderRadius: 28,
        background:'rgba(20,20,35,0.75)',
        backdropFilter:'blur(18px)', WebkitBackdropFilter:'blur(18px)',
        border:'1px solid rgba(237,231,220,0.08)',
      }}>
        {/* Waveform */}
        <div style={{ display:'flex', alignItems:'center', gap:2, height: 38, marginBottom: 10 }}>
          {Array.from({length:52}).map((_,i)=>{
            const h = 3 + Math.abs(Math.sin(i*0.55))*20 + Math.abs(Math.cos(i*0.3))*10;
            const played = i < 18;
            return (
              <div key={i} style={{
                flex:1, height: h, borderRadius: 1,
                background: played ? eco.accent : 'rgba(237,231,220,0.22)',
                boxShadow: played ? `0 0 4px ${eco.accent}` : 'none',
                opacity: played ? 0.9 : 0.6,
              }}/>
            );
          })}
        </div>
        <div style={{ display:'flex', alignItems:'center', gap: 14 }}>
          <div style={{
            width: 44, height: 44, borderRadius:'50%',
            background: eco.accent,
            boxShadow:`0 0 24px ${eco.accentGlow}`,
            display:'flex', alignItems:'center', justifyContent:'center',
          }}>
            <div style={{ display:'flex', gap: 3 }}>
              <div style={{ width:3, height:14, background:'#0a0a14' }}/>
              <div style={{ width:3, height:14, background:'#0a0a14' }}/>
            </div>
          </div>
          <div style={{ flex:1 }}>
            <div style={{
              fontFamily: eco.sans, fontSize: 12, color: eco.text, fontWeight: 500,
              marginBottom: 2,
            }}>Sta leggendo…</div>
            <div style={{
              fontFamily: eco.serif, fontStyle:'italic', fontSize: 12,
              color: eco.textFaint,
            }}>"sette settimane dal suo arrivo"</div>
          </div>
          <div style={{
            fontFamily: eco.mono, fontSize: 11, color: eco.textDim,
            padding:'4px 8px', borderRadius: 6,
            border: '1px solid rgba(237,231,220,0.12)',
          }}>1.0×</div>
        </div>
      </div>

      <EcoTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 3: REGISTRANDO ────────────────
function EcoRegistra() {
  return (
    <MPhone bg={eco.bg} statusColor={eco.text}>
      {/* Big living rings */}
      <div style={{ position:'absolute', top:'38%', left:'50%', transform:'translate(-50%, -50%)',
        zIndex:0,
      }}>
        <EcoRings size={540} intensity={1.3} />
      </div>
      <div style={{ position:'absolute', top:'38%', left:'50%', transform:'translate(-50%, -50%)',
        zIndex:1, width: 280, height: 280, borderRadius:'50%',
        background:'radial-gradient(circle, oklch(0.55 0.14 270 / 0.4), transparent 65%)',
        filter:'blur(20px)',
      }}/>

      <div style={{ position:'absolute', top: 82, left: 0, right: 0, zIndex: 3,
        textAlign:'center',
      }}>
        <div style={{
          display:'inline-flex', alignItems:'center', gap: 8,
          padding:'6px 14px', borderRadius: 999,
          background:'rgba(255,100,100,0.12)',
          border:'1px solid rgba(255,100,100,0.25)',
          fontFamily: eco.sans, fontSize: 10, letterSpacing: 2,
          color: '#ffaaaa', textTransform:'uppercase',
        }}>
          <div style={{ width:5, height:5, borderRadius:'50%', background:'#ff5555',
            boxShadow:'0 0 8px #ff5555' }}/>
          stai parlando
        </div>
      </div>

      {/* center — live transcript inside the rings */}
      <div style={{ position:'absolute', top:'38%', left:'50%', transform:'translate(-50%, -50%)',
        zIndex: 4, width: 280, textAlign:'center',
      }}>
        <div style={{
          fontFamily: eco.serif, fontSize: 20, lineHeight: 1.45, color: eco.text,
          fontStyle:'italic',
        }}>
          il tempo qui è <span style={{ color: eco.accent }}>elastico</span>,
          una lentezza
          <br/>
          <span style={{ color: eco.textDim }}>che si può abitare</span>
          <span style={{ display:'inline-block', width:2, height: 18, background: eco.accent,
            marginLeft: 4, verticalAlign:'middle',
            boxShadow:`0 0 6px ${eco.accent}` }}/>
        </div>
      </div>

      {/* Real-time waveform along bottom */}
      <div style={{
        position:'absolute', bottom: 200, left: 24, right: 24, zIndex: 3,
      }}>
        <div style={{ display:'flex', alignItems:'center', gap:2.5, height: 48,
          justifyContent:'center',
        }}>
          {Array.from({length: 42}).map((_,i)=>{
            const h = 4 + Math.abs(Math.sin(i*0.45 + 1))*30 + (i%4)*3;
            return (
              <div key={i} style={{
                width: 2.5, height: h, borderRadius: 1.5,
                background: eco.accent, opacity: 0.35 + Math.abs(Math.sin(i*0.3))*0.55,
                boxShadow: `0 0 3px ${eco.accent}`,
              }}/>
            );
          })}
        </div>
      </div>

      {/* Context at top — what you're annotating */}
      <div style={{ position:'absolute', top: 148, left: 26, right: 26, zIndex: 3,
        textAlign:'center',
      }}>
        <div style={{
          fontFamily: eco.mono, fontSize: 9, letterSpacing: 2,
          color: eco.textFaint, marginBottom: 6,
        }}>NOTA SU</div>
        <div style={{
          fontFamily: eco.serif, fontStyle:'italic', fontSize: 13,
          color: eco.textDim,
        }}>"…erano passate già sette settimane…"</div>
      </div>

      {/* Voice hints */}
      <div style={{ position:'absolute', bottom: 130, left: 0, right: 0, zIndex: 3,
        textAlign:'center',
      }}>
        <div style={{
          display:'inline-flex', gap: 18,
          fontFamily: eco.sans, fontSize: 10, letterSpacing: 0.5,
          color: eco.textFaint,
        }}>
          <span>"fatto" · salva</span>
          <span>·</span>
          <span>"rielabora" · riscrivi</span>
        </div>
      </div>

      <EcoTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 4: RIELABORAZIONE ────────────────
function EcoRielabora() {
  return (
    <MPhone bg={eco.bg} statusColor={eco.text}>
      <div style={{ position:'absolute', top:-60, right:-60, zIndex:0, opacity:0.45 }}>
        <EcoRings size={300} />
      </div>

      <div style={{ position:'absolute', top:54, left:0, right:0, zIndex:5,
        display:'flex', alignItems:'center', justifyContent:'space-between',
        padding:'14px 22px',
      }}>
        <div style={{
          fontFamily: eco.sans, fontSize: 10, letterSpacing: 2,
          color: eco.textFaint, textTransform:'uppercase',
        }}>Riscrittura</div>
        <div style={{
          display:'inline-flex', gap: 4, padding:'3px', borderRadius: 999,
          background:'rgba(237,231,220,0.05)',
          border:'1px solid rgba(237,231,220,0.08)',
        }}>
          <div style={{
            padding:'5px 12px', borderRadius: 999,
            fontFamily: eco.sans, fontSize: 11, color: '#0a0a14',
            background: eco.accent,
          }}>A/B</div>
          <div style={{
            padding:'5px 12px', borderRadius: 999,
            fontFamily: eco.sans, fontSize: 11, color: eco.textFaint,
          }}>solo nuovo</div>
        </div>
      </div>

      <div style={{ position:'relative', zIndex:1, padding:'108px 22px 170px',
        height:'100%', overflow:'hidden',
      }}>
        {/* A — Original */}
        <div style={{
          padding:'16px 18px', borderRadius: 16,
          background:'rgba(237,231,220,0.03)',
          border:'1px solid rgba(237,231,220,0.06)',
          marginBottom: 14,
        }}>
          <div style={{
            fontFamily: eco.mono, fontSize: 9, letterSpacing: 2,
            color: eco.textFaint, marginBottom: 10, textTransform:'uppercase',
          }}>A · originale</div>
          <div style={{
            fontFamily: eco.serif, fontSize: 16, lineHeight: 1.55,
            color: eco.textDim,
          }}>
            Hans Castorp osservava la neve cadere oltre il vetro, e pensava che erano passate già sette settimane dal suo arrivo.
          </div>
        </div>

        {/* voice note compact */}
        <div style={{
          display:'flex', alignItems:'center', gap: 10, padding:'10px 14px',
          borderRadius: 999,
          background:'rgba(237,231,220,0.04)',
          marginBottom: 14,
        }}>
          <div style={{ display:'flex', gap:1.5, height: 12 }}>
            {[4,7,5,9,6,8,5,7,10,6,4,6].map((h,i)=>(
              <div key={i} style={{ width:1.5, height:h, background: eco.accent, opacity:0.7 }}/>
            ))}
          </div>
          <div style={{
            flex:1,
            fontFamily: eco.serif, fontStyle:'italic', fontSize: 13,
            color: eco.text,
          }}>"rendilo più sensoriale, fai sentire la lentezza"</div>
          <div style={{ fontFamily: eco.mono, fontSize: 10, color: eco.textFaint }}>0:18</div>
        </div>

        {/* B — Rewritten */}
        <div style={{
          padding:'16px 18px', borderRadius: 16,
          background:'linear-gradient(145deg, rgba(70,50,140,0.22), rgba(20,20,35,0.4))',
          border:`1px solid ${eco.accent}`,
          boxShadow:`0 0 30px ${eco.accentGlow}`,
        }}>
          <div style={{
            fontFamily: eco.mono, fontSize: 9, letterSpacing: 2,
            color: eco.accent, marginBottom: 10, textTransform:'uppercase',
          }}>B · riscritto</div>
          <div style={{
            fontFamily: eco.serif, fontSize: 16, lineHeight: 1.6,
            color: eco.text,
          }}>
            La neve scendeva lenta oltre il vetro, e Hans Castorp la guardava cadere, fiocco dopo fiocco, sentendo come quelle sette settimane gli si fossero depositate addosso a strati.
          </div>
        </div>
      </div>

      {/* actions */}
      <div style={{
        position:'absolute', left:20, right:20, bottom: 96, zIndex:20,
        display:'flex', gap: 8,
      }}>
        <div style={{
          flex: 1, padding:'14px', borderRadius: 999,
          background:'rgba(237,231,220,0.05)',
          border:'1px solid rgba(237,231,220,0.08)',
          fontFamily: eco.sans, fontSize: 12, color: eco.textDim,
          textAlign:'center',
        }}>tieni A</div>
        <div style={{
          flex: 1.3, padding:'14px', borderRadius: 999,
          background: eco.accent, color:'#0a0a14',
          fontFamily: eco.sans, fontSize: 12, fontWeight: 600,
          textAlign:'center',
          boxShadow:`0 0 26px ${eco.accentGlow}`,
        }}>ascolta B ▸</div>
      </div>

      <EcoTabBar active="leggi" />
    </MPhone>
  );
}

// ──────────────── Screen 5: NOTE ────────────────
function EcoNote() {
  const notes = [
    { doc:'La montagna incantata', loc:'cap. III', text:'il tempo qui è elastico, una lentezza che si può abitare',
      dur:18, when:'oggi 16:04', st:'riscritto' },
    { doc:'La montagna incantata', loc:'cap. III', text:'confronta con l\'Ulisse — stream of consciousness',
      dur:24, when:'oggi 15:42', st:'' },
    { doc:'Lettera a Giulia — v4', loc:'§ 2', text:'togli il secondo paragrafo, più diretta',
      dur:9, when:'ieri', st:'applicato' },
    { doc:'Appunti sul Simposio', loc:'203a', text:'la traduzione di εἰς — qui non è verso, è dentro',
      dur:21, when:'ieri', st:'' },
    { doc:'La montagna incantata', loc:'cap. II', text:'il sanatorio come soglia — Joachim vs Castorp',
      dur:33, when:'mar', st:'riscritto' },
  ];
  return (
    <MPhone bg={eco.bg} statusColor={eco.text}>
      <div style={{ position:'absolute', top:-50, left:-80, zIndex:0, opacity:0.35 }}>
        <EcoRings size={280} />
      </div>

      <div style={{ position:'relative', zIndex:1, padding:'68px 24px 100px', height:'100%', overflow:'hidden' }}>
        <div style={{
          fontFamily: eco.sans, fontSize: 11, letterSpacing: 2.5,
          color: eco.textFaint, textTransform:'uppercase', marginBottom: 14,
        }}>Archivio vocale</div>
        <div style={{
          fontFamily: eco.serif, fontSize: 34, lineHeight: 1,
          color: eco.text, letterSpacing: -0.5, marginBottom: 6,
        }}>47 note</div>
        <div style={{
          fontFamily: eco.serif, fontStyle:'italic', fontSize: 14,
          color: eco.textDim, marginBottom: 22,
        }}>14 minuti di pensiero parlato</div>

        {/* Filter chips */}
        <div style={{ display:'flex', gap:8, marginBottom: 22, flexWrap:'wrap' }}>
          {['tutte','riscritte · 12','applicate · 8','in sospeso · 27'].map((c,i)=>(
            <div key={i} style={{
              padding:'6px 12px', borderRadius: 999,
              background: i===0 ? eco.accent : 'rgba(237,231,220,0.04)',
              border: i===0 ? 'none' : '1px solid rgba(237,231,220,0.08)',
              color: i===0 ? '#0a0a14' : eco.textDim,
              fontFamily: eco.sans, fontSize: 11, fontWeight: i===0 ? 600 : 400,
              letterSpacing: 0.2,
            }}>{c}</div>
          ))}
        </div>

        <div style={{ display:'flex', flexDirection:'column', gap: 16 }}>
          {notes.map((n, i)=>(
            <div key={i} style={{
              padding:'14px 16px', borderRadius: 14,
              background:'rgba(237,231,220,0.03)',
              border:'1px solid rgba(237,231,220,0.06)',
            }}>
              <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between',
                marginBottom: 8,
              }}>
                <div style={{ display:'flex', alignItems:'center', gap: 8 }}>
                  <div style={{
                    fontFamily: eco.sans, fontSize: 11, color: eco.text,
                    fontWeight: 500,
                  }}>{n.doc}</div>
                  <div style={{ width:3, height:3, borderRadius:'50%', background: eco.textFaint }}/>
                  <div style={{ fontFamily: eco.mono, fontSize: 10, color: eco.textFaint }}>{n.loc}</div>
                </div>
                <div style={{ fontFamily: eco.mono, fontSize: 9, color: eco.textFaint,
                  letterSpacing: 0.3,
                }}>{n.when}</div>
              </div>
              <div style={{
                fontFamily: eco.serif, fontStyle:'italic', fontSize: 15,
                lineHeight: 1.45, color: eco.textDim, marginBottom: 10,
              }}>"{n.text}"</div>
              <div style={{ display:'flex', alignItems:'center', gap: 10 }}>
                <div style={{
                  width: 26, height: 26, borderRadius:'50%',
                  border:`1px solid ${eco.accent}`,
                  display:'flex', alignItems:'center', justifyContent:'center',
                  flexShrink: 0,
                }}>
                  <div style={{ width:0, height:0,
                    borderLeft:`6px solid ${eco.accent}`,
                    borderTop:'4px solid transparent',
                    borderBottom:'4px solid transparent', marginLeft: 2,
                  }}/>
                </div>
                <div style={{ flex:1, display:'flex', alignItems:'center', gap:1.5, height:14 }}>
                  {Array.from({length:32}).map((_,j)=>{
                    const h = 3 + Math.abs(Math.sin(j*0.5 + i))*9;
                    return <div key={j} style={{ width:1.5, height:h, background: eco.accent,
                      opacity: 0.35+(j%3)*0.2 }}/>;
                  })}
                </div>
                <div style={{ fontFamily: eco.mono, fontSize: 10, color: eco.textFaint }}>0:{String(n.dur).padStart(2,'0')}</div>
                {n.st && (
                  <div style={{
                    fontFamily: eco.mono, fontSize: 9, letterSpacing: 0.5,
                    color: n.st==='applicato' ? eco.accent : eco.textDim,
                    textTransform:'uppercase',
                  }}>· {n.st}</div>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
      <EcoTabBar active="note" />
    </MPhone>
  );
}

// ──────────────── Screen 6: RIPRESA ────────────────
function EcoRipresa() {
  return (
    <MPhone bg={eco.bg} statusColor={eco.text}>
      <div style={{ position:'absolute', top:'45%', left:'50%', transform:'translate(-50%, -50%)',
        zIndex:0, opacity: 0.8,
      }}>
        <EcoRings size={560} intensity={0.9}/>
      </div>

      <div style={{ position:'absolute', top: 100, left: 0, right: 0, zIndex: 2,
        textAlign:'center', padding:'0 30px',
      }}>
        <div style={{
          fontFamily: eco.sans, fontSize: 11, letterSpacing: 3,
          color: eco.textFaint, textTransform:'uppercase', marginBottom: 18,
        }}>Bentornata</div>
        <div style={{
          fontFamily: eco.serif, fontSize: 28, lineHeight: 1.15,
          color: eco.text, letterSpacing: -0.4, marginBottom: 10,
        }}>La montagna<br/>incantata</div>
        <div style={{
          fontFamily: eco.serif, fontStyle:'italic', fontSize: 14,
          color: eco.textDim,
        }}>cap. III · 3h 58m di 11h 42m</div>
      </div>

      {/* Center: last heard */}
      <div style={{ position:'absolute', top: '45%', left: 28, right: 28, zIndex: 3,
        transform:'translateY(-50%)', textAlign:'center',
      }}>
        <div style={{
          fontFamily: eco.mono, fontSize: 9, letterSpacing: 2,
          color: eco.textFaint, marginBottom: 14, textTransform:'uppercase',
        }}>ultima frase</div>
        <div style={{
          fontFamily: eco.serif, fontSize: 19, lineHeight: 1.5,
          color: eco.text, fontStyle:'italic',
        }}>
          "…erano passate già
          <br/>
          <span style={{
            background: `linear-gradient(180deg, transparent 55%, ${eco.accentSoft} 55%)`,
          }}>sette settimane</span> dal suo arrivo…"
        </div>
      </div>

      {/* Big play button */}
      <div style={{ position:'absolute', bottom: 170, left: 0, right: 0, zIndex: 4,
        display:'flex', flexDirection:'column', alignItems:'center', gap: 16,
      }}>
        <div style={{ position:'relative' }}>
          <div style={{
            position:'absolute', inset:-18, borderRadius:'50%',
            border:`1px solid ${eco.accent}`, opacity: 0.35,
          }}/>
          <div style={{
            position:'absolute', inset:-36, borderRadius:'50%',
            border:`1px solid ${eco.accent}`, opacity: 0.18,
          }}/>
          <div style={{
            width: 92, height: 92, borderRadius:'50%',
            background: eco.accent,
            boxShadow:`0 0 40px ${eco.accentGlow}, 0 0 80px ${eco.accentGlow}`,
            display:'flex', alignItems:'center', justifyContent:'center',
          }}>
            <div style={{ width:0, height:0,
              borderLeft:'24px solid #0a0a14',
              borderTop:'16px solid transparent',
              borderBottom:'16px solid transparent', marginLeft: 7,
            }}/>
          </div>
        </div>
        <div style={{
          fontFamily: eco.serif, fontStyle:'italic', fontSize: 14,
          color: eco.textDim,
        }}>o di' "riprendi"</div>
      </div>

      <EcoTabBar active="leggi" />
    </MPhone>
  );
}

Object.assign(window, {
  EcoLibreria, EcoLeggi, EcoRegistra, EcoRielabora, EcoNote, EcoRipresa,
});
