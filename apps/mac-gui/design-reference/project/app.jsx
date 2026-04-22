// Main app — assembles the DesignCanvas with 3 variant columns × 6 screens.

const SCREENS = [
  { k:'libreria',   label:'01 · Libreria',              sub:'home / scaffale' },
  { k:'leggi',      label:'02 · Lettura',               sub:'documento aperto, TTS attivo' },
  { k:'registra',   label:'03 · Nota vocale',           sub:'sei interrotto, stai parlando' },
  { k:'rielabora',  label:'04 · Rielaborazione',        sub:'originale vs riscritto' },
  { k:'note',       label:'05 · Archivio note',         sub:'marginalia di tutti i documenti' },
  { k:'ripresa',    label:'06 · Ripresa',               sub:'torni all\'app, riparti dal punto' },
];

const VARIANTS = [
  { id:'notte',       name:'Notte',       tagline:'Più profonda e ambientale. Near-black cool, serif Lora, gradienti radiali morbidi, cromatura minima.',
    components: {
      libreria: () => <NotteLibreria />,
      leggi: () => <NotteLeggi />,
      registra: () => <NotteRegistra />,
      rielabora: () => <NotteRielabora />,
      note: () => <NotteNote />,
      ripresa: () => <NotteRipresa />,
  }},
  { id:'inchiostro',  name:'Inchiostro',  tagline:'La metafora del margine, letterale. Vellum scuro caldo, Cormorant corsivo, margine visibile con note ancorate da piccoli tick.',
    components: {
      libreria: () => <InchLibreria />,
      leggi: () => <InchLeggi />,
      registra: () => <InchRegistra />,
      rielabora: () => <InchRielabora />,
      note: () => <InchNote />,
      ripresa: () => <InchRipresa />,
  }},
  { id:'eco',         name:'Eco',         tagline:'L\'audio come presenza viva. Newsreader serif, onde concentriche, waveform come elemento UI primario, accento violaceo.',
    components: {
      libreria: () => <EcoLibreria />,
      leggi: () => <EcoLeggi />,
      registra: () => <EcoRegistra />,
      rielabora: () => <EcoRielabora />,
      note: () => <EcoNote />,
      ripresa: () => <EcoRipresa />,
  }},
];

function App() {
  return (
    <DesignCanvas>
      {/* Title block */}
      <div style={{ padding:'24px 60px 52px', maxWidth: 900 }}>
        <div style={{
          fontFamily:'Inter Tight, system-ui', fontSize:11, letterSpacing:2.5,
          color:'rgba(60,50,40,0.6)', textTransform:'uppercase', marginBottom: 14,
        }}>Marginalia · tre direzioni estetiche</div>
        <div style={{
          fontFamily:'Lora, Georgia, serif', fontSize: 44, lineHeight: 1.05,
          color:'rgba(40,30,20,0.9)', letterSpacing: -0.8, marginBottom: 14,
          fontWeight: 500,
        }}>Un reader vocale meditativo,<br/>tre interpretazioni.</div>
        <div style={{
          fontFamily:'Lora, Georgia, serif', fontStyle:'italic', fontSize: 18,
          color:'rgba(60,50,40,0.7)', lineHeight: 1.5, maxWidth: 720,
        }}>
          Tre direzioni per Marginalia, tutte dark-mode e tutte radicate nello stesso sistema (serif per leggere + sans UI + accento ink blue). Variano per temperatura, metafora visiva e ruolo dell'audio nell'interfaccia. Ogni colonna mostra sei stati chiave del flusso: libreria → lettura → interruzione vocale → rielaborazione → archivio note → ripresa.
        </div>
      </div>

      {/* One DCSection per variant (so each is h-stacked) */}
      {VARIANTS.map(v => (
        <DCSection key={v.id}
          title={v.name}
          subtitle={v.tagline}
          gap={40}>
          {SCREENS.map(s => (
            <DCArtboard key={s.k} label={s.label} width={390} height={844}
              style={{ background:'transparent', boxShadow:'none', borderRadius: 0 }}>
              {v.components[s.k]()}
            </DCArtboard>
          ))}
        </DCSection>
      ))}

      <DCPostIt top={560} left={30} rotate={-3} width={200}>
        Scorri orizzontalmente dentro ogni riga ↔
        <br/><br/>
        Zoom: pinch o ⌘+rotella.
      </DCPostIt>

    </DesignCanvas>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App/>);
