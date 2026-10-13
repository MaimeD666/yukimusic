// Deterministic UI contract fixture. Loaded only by navigation-smoke.py.
(() => {
  const clone = (v) => JSON.parse(JSON.stringify(v));
  const callbacks = new Map(), listeners = new Map();
  let callbackId = 0, key = 100;
  const tracks = Array.from({length: 90}, (_, i) => ({id: i + 1, title: i === 0 ? 'Облака' : `Track ${String(i + 1).padStart(3, '0')}`, artist: i % 2 ? 'Slow Air' : 'Луна', artistId: i % 2 ? 102 : 101, artwork: null, duration: 90 + i, permalink: `https://soundcloud.com/ui-test/track-${i}`, available: i !== 4}));
  const artists = [{id:101, name:'Луна', artwork:null, trackCount:16, permalink:'', artworkVerified:true}, {id:102, name:'Slow Air', artwork:null, trackCount:12, permalink:'', artworkVerified:true}];
  const playlists = [{id:201, title:'Ночная сторона', artist:'Луна', trackCount:12, artwork:null, permalink:''}, {id:202, title:'Cloud study', artist:'Slow Air', trackCount:12, artwork:null, permalink:''}];
  let library = {tracks:clone(tracks), artists, playlists};
  let state = {queue:tracks.slice(0,3).map((track,i)=>({key:`entry-${i}`,track})), currentKey:'entry-0', playing:false, loading:false, position:20, volume:.5, muted:false, shuffle:false, repeat:'off', eq:Array(10).fill(0), eqEnabled:false, error:null};
  const emit = () => {
    state = clone(state);
    for (const [id, {event, handler}] of listeners) if (event === 'playback') callbacks.get(handler)?.({event, id, payload:clone(state)});
  };
  const delay = (ms) => new Promise(resolve => setTimeout(resolve, ms));
  window.__uiTest = {calls:[], get state(){return clone(state)}, get library(){return clone(library)}, failSearch:false,
    spectrum(values) { for (const [id,{event,handler}] of listeners) if (event === 'spectrum') callbacks.get(handler)?.({event,id,payload:values}); }
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener:(_,id)=>listeners.delete(id)};
  window.__TAURI_INTERNALS__ = {
    transformCallback(fn) { callbacks.set(++callbackId, fn); return callbackId; },
    async invoke(command, args={}) {
      window.__uiTest.calls.push({command,args:clone(args)});
      if (command === 'plugin:event|listen') { const id = ++callbackId; listeners.set(id,args); return id; }
      if (command === 'plugin:event|unlisten') { listeners.delete(args.eventId); return; }
      if (command === 'snapshot') return clone(state);
      if (command === 'get_library') return clone(library);
      if (command === 'save_library') {library=clone(args.library); return;}
      if (command === 'artwork') return '';
      if (command === 'artist_profile') return clone(artists.find(a=>a.id===args.id) || artists[0]);
      if (command === 'artist_tracks') { await delay(120); return {tracks:clone(tracks.slice(args.next ? 8 : 0,args.next ? 16 : 10)),next:args.next?null:'artist-page-2',total:16}; }
      if (command === 'playlist_tracks') {await delay(80); return {tracks:clone(tracks.slice(0,12)),next:null,total:12};}
      if (command === 'search') {
        await delay(args.query === 'slow' ? 700 : 80);
        if (window.__uiTest.failSearch) throw new Error('Тестовая ошибка сети');
        return {tracks:clone(tracks.slice(0,30)).map(t=>({...t,title:`${args.query} · ${t.title}`})),artists:clone(artists),playlists:clone(playlists),next:{tracks:'next-tracks',artists:null,playlists:null},errors:[]};
      }
      if (command === 'search_more') return {tracks:clone(tracks.slice(30)),artists:[],playlists:[],next:{tracks:null,artists:null,playlists:null},errors:[]};
      if (command === 'window_action' || command === 'visualization') return;
      if (command === 'player_action') {
        const {action,value} = args;
        if (action === 'play') {state.queue=value.tracks.map(track=>({key:`entry-${++key}`,track})); state.currentKey=state.queue[value.index].key; state.playing=true; state.position=0;}
        if (action === 'toggle') state.playing=!state.playing;
        if (action === 'jump') {state.currentKey=value;state.position=0;state.playing=true;}
        if (action === 'seek') state.position=value;
        if (action === 'volume') state.volume=value;
        if (action === 'mute') state.muted=!state.muted;
        if (action === 'shuffle') state.shuffle=!state.shuffle;
        if (action === 'repeat') state.repeat=value;
        if (action === 'eq') {state.eq=value.bands;state.eqEnabled=value.enabled;}
        if (action === 'enqueue') {const entries=value.tracks.map(track=>({key:`entry-${++key}`,track})); state.queue.splice(value.next ? state.queue.findIndex(e=>e.key===state.currentKey)+1 : state.queue.length,0,...entries);}
        if (action === 'remove') state.queue=state.queue.filter(e=>e.key!==value);
        if (action === 'clear') state.queue=state.queue.filter(e=>e.key===state.currentKey);
        if (action === 'move') {const from=state.queue.findIndex(e=>e.key===value.from),to=state.queue.findIndex(e=>e.key===value.to); if(from>=0 && to>=0) state.queue.splice(to,0,state.queue.splice(from,1)[0]);}
        emit(); return;
      }
      throw new Error(`Unhandled fixture command: ${command}`);
    }
  };
})();
