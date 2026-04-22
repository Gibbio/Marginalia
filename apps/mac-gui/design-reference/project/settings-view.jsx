// Marginalia — Settings page (macOS desktop, Inchiostro)
// Reads marginalia.reference.toml + SETTINGS_SPEC.md as source of truth.
// Staged-state + Apply button pattern (the spec insists on this).

// ── design tokens from parent file (d, makeAccent) are global via scope ──

// Static reference data extracted from the spec. The real app discovers these
// at runtime — here they're frozen values to demonstrate the UI.
const LANGUAGES = [
  { code:'it-IT', display:'italiano', voices: 4 },
  { code:'en-US', display:'English (US)', voices: 6 },
  { code:'en-GB', display:'English (UK)', voices: 3 },
  { code:'fr-FR', display:'français', voices: 2 },
  { code:'es-ES', display:'español', voices: 2 },
  { code:'ja-JP', display:'日本語', voices: 1 },
];

const VOICES_ALL = [
  { id:'if_sara',    display:'Sara',    lang:'it-IT', gender:'female', backend:'mlx', installed: true },
  { id:'if_lucia',   display:'Lucia',   lang:'it-IT', gender:'female', backend:'mlx', installed: true },
  { id:'im_nicola',  display:'Nicola',  lang:'it-IT', gender:'male',   backend:'mlx', installed: true },
  { id:'im_marco',   display:'Marco',   lang:'it-IT', gender:'male',   backend:'mlx', installed: false },
  { id:'af_bella',   display:'Bella',   lang:'en-US', gender:'female', backend:'mlx', installed: false },
  { id:'am_adam',    display:'Adam',    lang:'en-US', gender:'male',   backend:'mlx', installed: false },
];

const STT_ENGINES = [
  { id:'apple',   name:'Apple Speech',  available: true,  reason: null,
    note: 'richiede macOS Dictation attivo (Sistema → Tastiera → Dettatura).' },
  { id:'whisper', name:'Whisper (ggml)', available: true,  reason: null,
    note: 'modello small.bin (465 MB) installato — offline.' },
];

const TTS_BACKENDS = [
  { id:'mlx',     name:'Kokoro MLX',     sub:'Apple Silicon · Metal', available: true, reason: null },
  { id:'kokoro',  name:'Kokoro ONNX',    sub:'fallback cross-platform', available: false,
    reason:'ONNX Runtime non installato' },
];

const VOICE_COMMANDS = [
  { action:'pause',        label:'Metti in pausa',          triggers:['pausa','ferma'] },
  { action:'resume',       label:'Riprendi',                triggers:['riprendi','continua'] },
  { action:'next',         label:'Chunk successivo',        triggers:['avanti','prossimo'] },
  { action:'back',         label:'Chunk precedente',        triggers:['indietro'] },
  { action:'repeat',       label:'Ripeti chunk',            triggers:['ripeti'] },
  { action:'stop',         label:'Ferma e riavvolgi',       triggers:['stop','basta'] },
  { action:'next_chapter', label:'Capitolo successivo',     triggers:['prossimo capitolo','capitolo avanti'] },
  { action:'prev_chapter', label:'Capitolo precedente',     triggers:['capitolo indietro','capitolo precedente'] },
  { action:'bookmark',     label:'Salva posizione',         triggers:['segna','segnalibro'] },
  { action:'note',         label:'Detta una nota',          triggers:['nota','appunto'] },
  { action:'where',        label:'Leggi posizione',         triggers:['dove sono','posizione'] },
];

const INSTALLATIONS = [
  { id:'mlx_it', label:'Kokoro MLX — voci italiane', size:'74 MB', installed: true, removable: false },
  { id:'whisper_small', label:'Whisper small (STT multilingua)', size:'465 MB', installed: true, removable: true },
  { id:'voice_sara',   label:'Voce: Sara (it, femminile)',   size:'0.5 MB', installed: true, removable: true },
  { id:'voice_lucia',  label:'Voce: Lucia (it, femminile)',  size:'0.5 MB', installed: true, removable: true },
  { id:'voice_nicola', label:'Voce: Nicola (it, maschile)',  size:'0.5 MB', installed: true, removable: true },
  { id:'voice_marco',  label:'Voce: Marco (it, maschile)',   size:'0.5 MB', installed: false, removable: false },
  { id:'voice_bella',  label:'Voce: Bella (en-US, femminile)', size:'0.5 MB', installed: false, removable: false },
  { id:'onnx',   label:'ONNX Runtime (TTS fallback)', size:'34 MB', installed: false, removable: false },
  { id:'pdfium', label:'PDFium (import PDF)',         size:'68 MB', installed: false, removable: false },
];

// Current / baseline spec — what's persisted in marginalia.toml.
const INITIAL_SPEC = {
  language: 'it-IT',
  ttsBackend: 'mlx',
  voice: 'if_sara',
  sttEngine: 'apple',
  chunkTargetChars: 300,
  sttDebug: true,
};

function SettingsView({ a, onClose }) {
  const [spec, setSpec] = React.useState(INITIAL_SPEC);
  const [saved, setSaved] = React.useState(INITIAL_SPEC);
  const [voiceCommands, setVoiceCommands] = React.useState(VOICE_COMMANDS);
  const [section, setSection] = React.useState('voice'); // sticky sidebar nav
  const [applying, setApplying] = React.useState(false);

  const dirty = JSON.stringify(spec) !== JSON.stringify(saved);
  const willSpawn = spec.sttEngine !== saved.sttEngine || spec.language !== saved.language;

  // When language changes, reset voice to first installed voice for that lang.
  const onLangChange = (lang) => {
    const next = { ...spec, language: lang };
    const voices = VOICES_ALL.filter(v => v.lang === lang && v.installed);
    if (!voices.some(v => v.id === spec.voice)) {
      next.voice = voices[0]?.id || '';
    }
    setSpec(next);
  };

  const onApply = () => {
    setApplying(true);
    setTimeout(() => {
      setSaved(spec);
      setApplying(false);
    }, 700);
  };

  const scrollRef = React.useRef(null);
  const sectionRefs = {
    language: React.useRef(null),
    voice: React.useRef(null),
    stt: React.useRef(null),
    commands: React.useRef(null),
    audio: React.useRef(null),
    installations: React.useRef(null),
    diagnostics: React.useRef(null),
  };

  const goTo = (key) => {
    setSection(key);
    const el = sectionRefs[key].current;
    if (el && scrollRef.current) {
      scrollRef.current.scrollTo({ top: el.offsetTop - 40, behavior: 'smooth' });
    }
  };

  return (
    <div style={{ flex: 1, display:'flex', flexDirection:'column',
      position:'relative', overflow:'hidden', background: d.bg,
    }}>
      {/* Top bar: back · title · Apply */}
      <div style={{ height: 52, padding:'0 24px', display:'flex', alignItems:'center',
        justifyContent:'space-between', borderBottom:`1px solid ${d.line}`,
        position:'relative', zIndex: 3, flexShrink: 0,
      }}>
        <div style={{ display:'flex', alignItems:'center', gap: 14 }}>
          <div onClick={onClose} style={{
            display:'flex', alignItems:'center', gap:6, cursor:'pointer',
            padding:'6px 10px 6px 6px', borderRadius:6,
            color: d.textDim, fontFamily: d.sans, fontSize: 12,
          }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <path d="M15 18l-6-6 6-6"/>
            </svg>
            <span>torna alla lettura</span>
          </div>
          <div style={{ width:1, height: 16, background: d.line }}/>
          <div style={{ fontFamily: d.serif, fontSize: 20, fontStyle:'italic', color: d.text }}>
            Impostazioni
          </div>
        </div>

        <div style={{ display:'flex', alignItems:'center', gap: 14 }}>
          {dirty && (
            <div style={{ fontFamily: d.mono, fontSize: 10, color: a.accent, letterSpacing: 0.5 }}>
              modifiche in sospeso{willSpawn && ' · riavvia motore'}
            </div>
          )}
          <div onClick={() => dirty && !applying && onApply()}
            style={{
              padding:'7px 16px', borderRadius: 7,
              fontFamily: d.sans, fontSize: 12, fontWeight: 500,
              background: dirty ? a.accent : 'rgba(239,229,207,0.08)',
              color: dirty ? '#0f0e10' : d.textFaint,
              border: `1px solid ${dirty ? a.accent : d.line}`,
              cursor: dirty && !applying ? 'pointer' : 'default',
              boxShadow: dirty ? `0 0 20px ${a.accentGlow}` : 'none',
              display:'flex', alignItems:'center', gap: 8,
              transition: 'all 0.15s ease',
            }}>
            {applying && (
              <div style={{ width: 10, height: 10, borderRadius:'50%',
                border:`1.5px solid rgba(15,14,16,0.3)`, borderTopColor:'#0f0e10',
                animation: 'spin 0.7s linear infinite',
              }}/>
            )}
            <span>{applying ? 'Applicazione…' : 'Applica'}</span>
          </div>
        </div>
      </div>

      <div style={{ display:'flex', flex:1, overflow:'hidden' }}>
        {/* sub-nav */}
        <div style={{ width: 200, borderRight: `1px solid ${d.line}`, padding: '18px 0',
          background: 'rgba(0,0,0,0.12)', flexShrink: 0,
        }}>
          {[
            ['language', 'Lingua'],
            ['voice', 'Voce'],
            ['stt', 'Riconoscimento vocale'],
            ['commands', 'Comandi vocali'],
            ['audio', 'Audio & lettura'],
            ['installations', 'Installazioni'],
            ['diagnostics', 'Diagnostica'],
          ].map(([k, l]) => {
            const active = section === k;
            return (
              <div key={k} onClick={()=>goTo(k)} style={{
                padding:'8px 20px', cursor:'pointer', position:'relative',
                fontFamily: d.serif, fontStyle: active ? 'italic' : 'normal',
                fontSize: 14, color: active ? d.text : d.textDim,
                background: active ? 'rgba(255,255,255,0.04)' : 'transparent',
                borderLeft: active ? `2px solid ${a.accent}` : '2px solid transparent',
              }}>
                {l}
              </div>
            );
          })}

          <div style={{ height: 1, background: d.line, margin: '14px 20px' }}/>
          <div style={{ padding: '6px 20px', fontFamily: d.mono, fontSize: 9,
            color: d.textFaint, letterSpacing: 1.2, textTransform:'uppercase', marginBottom: 4,
          }}>file</div>
          <div style={{ padding:'0 20px' }}>
            <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textDim, lineHeight: 1.6 }}>
              ~/Library/App&nbsp;Support/<br/>Marginalia/<span style={{ color: d.textFaint }}>marginalia.toml</span>
            </div>
          </div>
        </div>

        {/* content */}
        <div ref={scrollRef} className="no-scrollbar" style={{
          flex: 1, overflow:'auto', padding: '24px 48px 80px',
          position:'relative',
        }}>
          <div style={{ maxWidth: 720 }}>

            {/* LANGUAGE */}
            <Section refEl={sectionRefs.language} kicker="001" title="Lingua"
              sub="determina le voci disponibili e la lingua di default del riconoscimento.">
              <LangPicker
                value={spec.language}
                onChange={onLangChange}
                a={a}
              />
              {(() => {
                const installedVoices = VOICES_ALL.filter(v => v.lang === spec.language && v.installed);
                if (installedVoices.length === 0) {
                  return (
                    <HintCard a={a}>
                      Nessuna voce installata per questa lingua.
                      <span style={{ color: a.accent, marginLeft: 8, cursor:'pointer',
                        textDecoration:'underline', textUnderlineOffset: 3 }}>Installa voci…</span>
                    </HintCard>
                  );
                }
                return null;
              })()}
            </Section>

            {/* VOICE */}
            <Section refEl={sectionRefs.voice} kicker="002" title="Voce"
              sub="la voce usata per leggerti il testo ad alta voce.">
              <VoicePicker
                spec={spec}
                onChange={v => setSpec({ ...spec, voice: v })}
                a={a}
              />
              <div style={{
                marginTop: 18, padding:'14px 16px', borderRadius: 10,
                background:'rgba(255,255,255,0.02)', border:`1px solid ${d.line}`,
                display:'flex', alignItems:'center', gap: 16,
              }}>
                <PlayButton a={a}/>
                <div style={{ flex: 1 }}>
                  <div style={{ fontFamily: d.mono, fontSize: 10, letterSpacing: 1.5,
                    color: d.textFaint, textTransform:'uppercase', marginBottom: 3,
                  }}>Anteprima</div>
                  <div style={{ fontFamily: d.serif, fontSize: 15, color: d.text,
                    fontStyle:'italic', lineHeight: 1.5,
                  }}>"Il tempo, nell'alta montagna, non è il tempo della pianura."</div>
                </div>
                {/* waveform */}
                <div style={{ display:'flex', alignItems:'center', gap: 1.5, height: 22 }}>
                  {Array.from({length: 30}).map((_,i)=>{
                    const h = 3 + Math.abs(Math.sin(i*0.55))*14;
                    return <div key={i} style={{ width:1.5, height:h, background: d.textFaint,
                      opacity: 0.3 + (i%3)*0.2 }}/>;
                  })}
                </div>
              </div>
            </Section>

            {/* STT */}
            <Section refEl={sectionRefs.stt} kicker="003" title="Riconoscimento vocale"
              sub="ascolta i tuoi comandi e registra le note dettate.">
              <SttPicker
                spec={spec}
                onChange={id => setSpec({ ...spec, sttEngine: id })}
                a={a}
              />
              <div style={{ marginTop: 16, display:'grid', gridTemplateColumns:'1fr 1fr', gap: 12 }}>
                <MiniStat label="Comandi · timeout silenzio" value="0.8 s"/>
                <MiniStat label="Comandi · durata max" value="4.0 s"/>
                <MiniStat label="Dettatura · timeout silenzio" value="1.5 s"/>
                <MiniStat label="Dettatura · durata max" value="60 s"/>
              </div>
              <Checkbox
                checked={spec.sttDebug}
                onChange={v => setSpec({ ...spec, sttDebug: v })}
                label="Mostra trascrizione grezza nel log"
                a={a}
              />
            </Section>

            {/* VOICE COMMANDS */}
            <Section refEl={sectionRefs.commands} kicker="004" title="Comandi vocali"
              sub="parole che, se pronunciate, attivano un'azione. le modifiche si salvano al volo.">
              <div style={{ borderTop:`1px solid ${d.line}` }}>
                {voiceCommands.map((cmd, i) => (
                  <CommandRow key={cmd.action}
                    cmd={cmd}
                    onChange={(triggers) => {
                      const next = [...voiceCommands];
                      next[i] = { ...cmd, triggers };
                      setVoiceCommands(next);
                    }}
                    a={a}
                  />
                ))}
              </div>
            </Section>

            {/* AUDIO */}
            <Section refEl={sectionRefs.audio} kicker="005" title="Audio & lettura"
              sub="come il testo viene spezzettato e dove l'audio viene memorizzato.">
              <div style={{ marginBottom: 22 }}>
                <ControlLabel>Dimensione chunk <span style={{ color: d.textFaint }}>· {spec.chunkTargetChars} caratteri</span></ControlLabel>
                <input type="range" min="100" max="1000" step="50"
                  value={spec.chunkTargetChars}
                  onChange={e => setSpec({ ...spec, chunkTargetChars: Number(e.target.value) })}
                  style={{ width: '100%', accentColor: a.accent }}
                />
                <div style={{ display:'flex', justifyContent:'space-between',
                  fontFamily: d.mono, fontSize: 9, color: d.textFaint, marginTop: 6,
                }}>
                  <span>100 · più navigabile, sintesi rapida</span>
                  <span>1000 · ascolto continuo</span>
                </div>
                {spec.chunkTargetChars !== saved.chunkTargetChars && (
                  <div style={{ marginTop: 10, fontFamily: d.serif, fontStyle:'italic',
                    fontSize: 12, color: a.accent,
                  }}>
                    ⚠ il cambio richiederà di re-importare i documenti esistenti.
                  </div>
                )}
              </div>

              <PathRow
                label="Libreria"
                value=".marginalia/beta.sqlite3"
                tag="SQLite"
              />
              <PathRow
                label="Cache audio"
                value=".marginalia/tts-cache"
                tag="FLAC · 1 per chunk"
                action="svuota"
                a={a}
              />
            </Section>

            {/* INSTALLATIONS */}
            <Section refEl={sectionRefs.installations} kicker="006" title="Installazioni"
              sub="modelli e voci sul disco. questa è l'unica sezione che scarica dalla rete.">
              <OnlineBanner a={a}/>
              <div style={{ marginTop: 14, border: `1px solid ${d.line}`, borderRadius: 10, overflow:'hidden' }}>
                {INSTALLATIONS.map((it, i) => (
                  <InstallRow key={it.id} item={it} last={i===INSTALLATIONS.length-1} a={a}/>
                ))}
              </div>
            </Section>

            {/* DIAGNOSTICS */}
            <Section refEl={sectionRefs.diagnostics} kicker="007" title="Diagnostica"
              sub="informazioni utili per il supporto — sola lettura.">
              <DiagTable spec={spec}/>
            </Section>

          </div>
        </div>
      </div>
    </div>
  );
}

// ── Section shell ──
function Section({ refEl, kicker, title, sub, children }) {
  return (
    <section ref={refEl} style={{ marginBottom: 56, scrollMarginTop: 40 }}>
      <div style={{ marginBottom: 22 }}>
        <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textFaint,
          letterSpacing: 2, marginBottom: 8,
        }}>{kicker}</div>
        <div style={{ fontFamily: d.serif, fontSize: 32, color: d.text,
          letterSpacing: -0.3, marginBottom: 6, fontWeight: 500,
        }}>{title}</div>
        <div style={{ fontFamily: d.serif, fontStyle:'italic', fontSize: 15,
          color: d.textDim, maxWidth: 560, lineHeight: 1.5,
        }}>{sub}</div>
      </div>
      {children}
    </section>
  );
}

function ControlLabel({ children }) {
  return <div style={{
    fontFamily: 'Inter Tight, system-ui', fontSize: 11, color: d.textDim,
    letterSpacing: 0.5, textTransform:'uppercase', marginBottom: 10,
  }}>{children}</div>;
}

function HintCard({ children, a }) {
  return (
    <div style={{ marginTop: 14, padding: '12px 14px', borderRadius: 8,
      background: `oklch(0.7 0.14 250 / 0.08)`,
      border: `1px solid oklch(0.7 0.14 250 / 0.25)`,
      fontFamily: d.serif, fontStyle:'italic', fontSize: 13, color: d.text,
    }}>{children}</div>
  );
}

// ── LANGUAGE picker: pill grid ──
function LangPicker({ value, onChange, a }) {
  return (
    <div style={{ display:'grid', gridTemplateColumns:'repeat(3, 1fr)', gap: 8 }}>
      {LANGUAGES.map(l => {
        const active = value === l.code;
        return (
          <div key={l.code} onClick={()=>onChange(l.code)} style={{
            padding:'12px 14px', borderRadius: 10, cursor:'pointer',
            background: active ? 'rgba(255,255,255,0.05)' : 'rgba(255,255,255,0.015)',
            border: `1px solid ${active ? a.accent : d.line}`,
            boxShadow: active ? `0 0 18px ${a.accentGlow}` : 'none',
            transition: 'all 0.15s',
          }}>
            <div style={{ fontFamily: d.serif, fontSize: 16, color: active ? d.text : d.textDim,
              fontStyle: active ? 'italic' : 'normal',
            }}>{l.display}</div>
            <div style={{ display:'flex', justifyContent:'space-between', marginTop: 4 }}>
              <div style={{ fontFamily: d.mono, fontSize: 9, color: d.textFaint }}>{l.code}</div>
              <div style={{ fontFamily: d.mono, fontSize: 9,
                color: active ? a.accent : d.textFaint,
              }}>{l.voices} voci</div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ── VOICE picker: list with gender icons ──
function VoicePicker({ spec, onChange, a }) {
  const voices = VOICES_ALL.filter(v => v.lang === spec.language);
  if (voices.length === 0) {
    return <div style={{ fontFamily: d.serif, fontStyle:'italic', color: d.textFaint, padding: 20 }}>
      nessuna voce disponibile per {spec.language}
    </div>;
  }
  return (
    <div style={{ border: `1px solid ${d.line}`, borderRadius: 10, overflow:'hidden' }}>
      {voices.map((v, i) => {
        const active = spec.voice === v.id;
        const last = i === voices.length - 1;
        return (
          <div key={v.id}
            onClick={()=> v.installed && onChange(v.id)}
            style={{
              display:'flex', alignItems:'center', gap: 14,
              padding:'14px 16px',
              background: active ? 'rgba(255,255,255,0.04)' : 'transparent',
              borderBottom: last ? 'none' : `1px solid ${d.lineSoft}`,
              borderLeft: active ? `2px solid ${a.accent}` : '2px solid transparent',
              opacity: v.installed ? 1 : 0.45,
              cursor: v.installed ? 'pointer' : 'not-allowed',
            }}>
            <GenderIcon g={v.gender}/>
            <div style={{ flex: 1 }}>
              <div style={{ fontFamily: d.serif, fontSize: 16, color: d.text,
                fontStyle: active ? 'italic' : 'normal',
              }}>{v.display}</div>
              <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textFaint,
                letterSpacing: 0.3, marginTop: 2,
              }}>{v.id} · {v.backend}</div>
            </div>
            {!v.installed && (
              <div style={{ fontFamily: d.mono, fontSize: 10, color: d.textFaint,
                padding:'3px 8px', borderRadius: 4, border: `1px solid ${d.line}`,
              }}>non installata</div>
            )}
            {active && (
              <div style={{ width: 8, height: 8, borderRadius:'50%', background: a.accent,
                boxShadow:`0 0 10px ${a.accent}` }}/>
            )}
          </div>
        );
      })}
    </div>
  );
}

function GenderIcon({ g }) {
  const color = d.textDim;
  if (g === 'female') return (
    <svg width="16" height="22" viewBox="0 0 16 22" fill="none" stroke={color} strokeWidth="1.5">
      <circle cx="8" cy="5" r="3"/><path d="M8 8v3l-3 7h6l-3-7"/>
    </svg>
  );
  return (
    <svg width="16" height="22" viewBox="0 0 16 22" fill="none" stroke={color} strokeWidth="1.5">
      <circle cx="8" cy="5" r="3"/><path d="M8 8v10M5 18h6M5 13h6"/>
    </svg>
  );
}

function PlayButton({ a }) {
  return (
    <div style={{
      width: 40, height: 40, borderRadius:'50%', background: a.accent,
      display:'flex', alignItems:'center', justifyContent:'center',
      boxShadow:`0 0 20px ${a.accentGlow}`, cursor:'pointer',
    }}>
      <div style={{ width:0, height:0,
        borderLeft:'10px solid #0f0e10',
        borderTop:'6px solid transparent',
        borderBottom:'6px solid transparent',
        marginLeft: 3,
      }}/>
    </div>
  );
}

// ── STT engine picker — segmented with availability reasons ──
function SttPicker({ spec, onChange, a }) {
  return (
    <div>
      <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', gap: 10 }}>
        {STT_ENGINES.map(eng => {
          const active = spec.sttEngine === eng.id;
          return (
            <div key={eng.id}
              onClick={()=> eng.available && onChange(eng.id)}
              style={{
                padding: 16, borderRadius: 10,
                background: active ? 'rgba(255,255,255,0.04)' : 'rgba(255,255,255,0.015)',
                border: `1px solid ${active ? a.accent : d.line}`,
                boxShadow: active ? `0 0 18px ${a.accentGlow}` : 'none',
                opacity: eng.available ? 1 : 0.45,
                cursor: eng.available ? 'pointer' : 'not-allowed',
              }}>
              <div style={{ display:'flex', alignItems:'center', gap: 8, marginBottom: 8 }}>
                <div style={{ width: 8, height: 8, borderRadius:'50%',
                  background: active ? a.accent : d.textGhost,
                  boxShadow: active ? `0 0 10px ${a.accent}` : 'none',
                }}/>
                <div style={{ fontFamily: d.serif, fontSize: 16, color: d.text,
                  fontStyle: active ? 'italic' : 'normal',
                }}>{eng.name}</div>
              </div>
              <div style={{ fontFamily: d.serif, fontSize: 13, fontStyle:'italic',
                color: d.textDim, lineHeight: 1.5,
              }}>{eng.note}</div>
              {!eng.available && eng.reason && (
                <div style={{ marginTop: 8, fontFamily: d.mono, fontSize: 10,
                  color: d.textFaint,
                }}>{eng.reason}</div>
              )}
            </div>
          );
        })}
      </div>

      {spec.sttEngine === 'apple' && (
        <HintCard a={a}>
          Apple Speech richiede la dettatura macOS.
          <span onClick={()=>{}} style={{ color: a.accent, marginLeft: 10, cursor:'pointer',
            textDecoration:'underline', textUnderlineOffset: 3 }}>Apri Impostazioni di sistema →</span>
        </HintCard>
      )}
    </div>
  );
}

function MiniStat({ label, value }) {
  return (
    <div style={{ padding:'12px 14px', borderRadius: 8,
      background:'rgba(255,255,255,0.02)', border: `1px solid ${d.line}`,
    }}>
      <div style={{ fontFamily: d.mono, fontSize: 9, color: d.textFaint,
        letterSpacing: 1.2, textTransform:'uppercase', marginBottom: 4,
      }}>{label}</div>
      <div style={{ fontFamily: d.serif, fontSize: 18, color: d.text }}>{value}</div>
    </div>
  );
}

function Checkbox({ checked, onChange, label, a }) {
  return (
    <div onClick={()=>onChange(!checked)} style={{
      marginTop: 16, display:'flex', alignItems:'center', gap: 10, cursor:'pointer',
    }}>
      <div style={{
        width: 16, height: 16, borderRadius: 4,
        background: checked ? a.accent : 'transparent',
        border: `1px solid ${checked ? a.accent : d.textGhost}`,
        display:'flex', alignItems:'center', justifyContent:'center',
      }}>
        {checked && (
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="#0f0e10" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
        )}
      </div>
      <div style={{ fontFamily: d.serif, fontSize: 14, color: d.textDim }}>{label}</div>
    </div>
  );
}

// ── Voice command row (editable chip list) ──
function CommandRow({ cmd, onChange, a }) {
  const [draft, setDraft] = React.useState('');
  const removeAt = (idx) => onChange(cmd.triggers.filter((_, i) => i !== idx));
  const addTrigger = () => {
    const v = draft.trim();
    if (!v || cmd.triggers.includes(v)) { setDraft(''); return; }
    onChange([...cmd.triggers, v]);
    setDraft('');
  };

  return (
    <div style={{
      display:'flex', alignItems:'flex-start', gap: 20,
      padding:'14px 0',
      borderBottom:`1px solid ${d.lineSoft}`,
    }}>
      <div style={{ width: 200, flexShrink: 0, paddingTop: 6 }}>
        <div style={{ fontFamily: d.serif, fontSize: 15, color: d.text }}>{cmd.label}</div>
        <div style={{ fontFamily: d.mono, fontSize: 9, color: d.textFaint,
          letterSpacing: 0.3, marginTop: 3,
        }}>{cmd.action}</div>
      </div>
      <div style={{ flex: 1, display:'flex', flexWrap:'wrap', gap: 6, alignItems:'center' }}>
        {cmd.triggers.map((t, i) => (
          <div key={t + i} style={{
            display:'inline-flex', alignItems:'center', gap: 6,
            padding:'5px 8px 5px 10px', borderRadius: 6,
            background: 'rgba(239,229,207,0.05)',
            border: `1px solid ${d.line}`,
            fontFamily: d.serif, fontStyle:'italic', fontSize: 13, color: d.text,
          }}>
            "{t}"
            <div onClick={()=>removeAt(i)} style={{ cursor:'pointer',
              width: 14, height: 14, borderRadius: 3,
              display:'flex', alignItems:'center', justifyContent:'center',
              color: d.textFaint,
            }}>×</div>
          </div>
        ))}
        <input
          type="text"
          placeholder="+ aggiungi"
          value={draft}
          onChange={e => setDraft(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') addTrigger(); }}
          onBlur={addTrigger}
          style={{
            background:'transparent', border: `1px dashed ${d.textGhost}`,
            padding:'5px 10px', borderRadius: 6,
            fontFamily: d.serif, fontSize: 13, fontStyle:'italic',
            color: d.text, width: 120, outline:'none',
          }}
        />
      </div>
    </div>
  );
}

// ── Path row ──
function PathRow({ label, value, tag, action, a }) {
  return (
    <div style={{ display:'flex', alignItems:'center', gap: 14,
      padding:'14px 16px', marginBottom: 10, borderRadius: 10,
      background:'rgba(255,255,255,0.02)', border: `1px solid ${d.line}`,
    }}>
      <div style={{ width: 140, flexShrink: 0 }}>
        <div style={{ fontFamily: 'Inter Tight', fontSize: 11, color: d.textDim,
          letterSpacing: 0.5, textTransform:'uppercase',
        }}>{label}</div>
        <div style={{ fontFamily: d.mono, fontSize: 9, color: d.textFaint, marginTop: 3 }}>{tag}</div>
      </div>
      <div style={{ flex: 1, fontFamily: d.mono, fontSize: 12, color: d.text,
        overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap',
      }}>{value}</div>
      <div style={{ display:'flex', gap: 6 }}>
        <IconBtn label="sfoglia"/>
        {action && <IconBtn label={action} danger a={a}/>}
      </div>
    </div>
  );
}

function IconBtn({ label, danger, a }) {
  return (
    <div style={{
      padding:'5px 10px', borderRadius: 6,
      fontFamily: 'Inter Tight', fontSize: 11,
      color: danger ? '#e88' : d.textDim,
      border: `1px solid ${danger ? 'rgba(232,136,136,0.3)' : d.line}`,
      background:'transparent', cursor:'pointer',
    }}>{label}</div>
  );
}

// ── Online banner ──
function OnlineBanner({ a }) {
  return (
    <div style={{ padding:'12px 16px', borderRadius: 10,
      background: `oklch(0.7 0.14 250 / 0.06)`,
      border: `1px solid oklch(0.7 0.14 250 / 0.2)`,
      display:'flex', alignItems:'center', gap: 12,
    }}>
      <div style={{ width: 28, height: 28, borderRadius:'50%',
        background:'rgba(239,229,207,0.06)', border:`1px solid ${d.textGhost}`,
        display:'flex', alignItems:'center', justifyContent:'center',
        flexShrink: 0,
      }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none"
          stroke={a.accent} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
        </svg>
      </div>
      <div style={{ flex: 1, fontFamily: d.serif, fontSize: 14, fontStyle:'italic',
        color: d.textDim, lineHeight: 1.5,
      }}>
        Marginalia funziona completamente offline.
        Questa è l'unica sezione che effettua chiamate di rete —
        i download vengono da <span style={{ color: d.text }}>huggingface.co</span> e <span style={{ color: d.text }}>github.com</span>.
      </div>
    </div>
  );
}

// ── Install row ──
function InstallRow({ item, last, a }) {
  const isVoice = item.id.startsWith('voice_');
  return (
    <div style={{ display:'flex', alignItems:'center', gap: 14,
      padding:'14px 16px',
      background: 'rgba(255,255,255,0.015)',
      borderBottom: last ? 'none' : `1px solid ${d.lineSoft}`,
    }}>
      <div style={{ width: 24, height: 24, borderRadius: 6,
        background: item.installed ? 'oklch(0.7 0.14 250 / 0.15)' : 'rgba(255,255,255,0.03)',
        border: `1px solid ${item.installed ? 'oklch(0.7 0.14 250 / 0.35)' : d.line}`,
        display:'flex', alignItems:'center', justifyContent:'center', flexShrink: 0,
      }}>
        {item.installed ? (
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={a.accent}
            strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
        ) : isVoice ? (
          <svg width="10" height="14" viewBox="0 0 16 22" fill="none" stroke={d.textFaint} strokeWidth="1.5">
            <circle cx="8" cy="5" r="3"/><path d="M8 8v10"/>
          </svg>
        ) : (
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke={d.textFaint}
            strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="7 13 12 18 17 13"/><path d="M12 2v16"/>
          </svg>
        )}
      </div>
      <div style={{ flex: 1 }}>
        <div style={{ fontFamily: d.serif, fontSize: 14, color: d.text,
          fontStyle: item.installed ? 'italic' : 'normal',
        }}>{item.label}</div>
        <div style={{ fontFamily: d.mono, fontSize: 9, color: d.textFaint, marginTop: 2,
          letterSpacing: 0.3,
        }}>{item.size} · {item.installed ? 'installato' : 'non installato'}</div>
      </div>
      <div>
        {item.installed && item.removable && (
          <IconBtn label="rimuovi" danger/>
        )}
        {!item.installed && (
          <div style={{ padding:'5px 12px', borderRadius: 6,
            fontFamily: 'Inter Tight', fontSize: 11, fontWeight: 500,
            color: a.accent,
            border: `1px solid oklch(0.7 0.14 250 / 0.45)`,
            background: `oklch(0.7 0.14 250 / 0.1)`,
            cursor:'pointer',
          }}>installa</div>
        )}
      </div>
    </div>
  );
}

// ── Diagnostics ──
function DiagTable({ spec }) {
  const rows = [
    ['TTS backend',     'kokoro-mlx'],
    ['TTS voice',       spec.voice],
    ['TTS model path',  '~/Library/.../models/tts/mlx/kokoro-v1_0.safetensors'],
    ['STT engine',      spec.sttEngine],
    ['STT language',    spec.language],
    ['Whisper model',   '~/Library/.../models/stt/whisper/ggml-small.bin'],
    ['Cache',           '~/Library/.../tts-cache · 142 MB · 1.247 file'],
    ['Ultimo apply',    '2026-04-20 14:22:08 · 420 ms · stt_swapped=true'],
    ['Versione app',    'Marginalia 0.9.2 (beta)'],
    ['Runtime',         'aarch64 · macOS 26.1 · Metal 4'],
  ];
  return (
    <div style={{ border:`1px solid ${d.line}`, borderRadius: 10, overflow:'hidden' }}>
      {rows.map(([k, v], i) => (
        <div key={k} style={{
          display:'flex', padding:'10px 16px',
          background: i%2 ? 'transparent' : 'rgba(255,255,255,0.015)',
          borderBottom: i === rows.length-1 ? 'none' : `1px solid ${d.lineSoft}`,
        }}>
          <div style={{ width: 180, fontFamily: 'Inter Tight', fontSize: 11,
            color: d.textDim, letterSpacing: 0.5, textTransform:'uppercase',
            paddingTop: 2,
          }}>{k}</div>
          <div style={{ flex: 1, fontFamily: d.mono, fontSize: 11, color: d.text,
            wordBreak:'break-all',
          }}>{v}</div>
        </div>
      ))}
    </div>
  );
}

window.SettingsView = SettingsView;
