<script lang="ts">
  import { reveal } from "../lib/motion";
  export let name = "";
  export let size = 20;
  const paths: Record<string, string> = {
    cloud: "M7 18a5 5 0 1 1 .5-10A6 6 0 0 1 19 9a4.5 4.5 0 0 1-.5 9H7Z",
    library:
      "M4 5v14M9 5v14m5-14 5 14M3 5h2M8 5h2m3 0 2-.5M3 19h2M8 19h2m9 0 2-.5",
    artist:
      "M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM4 21v-2a6 6 0 0 1 6-6h4a6 6 0 0 1 6 6v2",
    settings:
      "M9.5 3h5l.5 2.4 1.5.9 2.4-.8 2.5 4.3-1.8 1.6v1.2l1.8 1.6-2.5 4.3-2.4-.8-1.5.9-.5 2.4h-5L9 18.6l-1.5-.9-2.4.8-2.5-4.3 1.8-1.6v-1.2L2.6 9.8l2.5-4.3 2.4.8L9 5.4 9.5 3ZM15.25 12a3.25 3.25 0 1 1-6.5 0 3.25 3.25 0 0 1 6.5 0Z",
    sun: "M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5",
    moon: "M20.5 13A9 9 0 0 1 11 3.5 9 9 0 1 0 20.5 13Z",
    check: "m5 12 4 4L19 6",
    more: "M6 12a1 1 0 1 1-2 0 1 1 0 0 1 2 0Zm7 0a1 1 0 1 1-2 0 1 1 0 0 1 2 0Zm7 0a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z",
    music:
      "M9 17V5l11-2v12M9 9l11-2M9 17a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm11-2a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
    filter: "M4 7h16M7 12h10m-7 5h4",
    clock: "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM12 7v5l3 2",
    up: "m5 15 7-7 7 7",
    play: "M8 5l11 7-11 7V5Z",
    pause: "M8 5v14M16 5v14",
    next: "M5 5l10 7-10 7V5ZM19 5v14",
    previous: "M19 5 9 12l10 7V5ZM5 5v14",
    search: "M10.5 18a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15ZM16 16l5 5",
    heart:
      "M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 21l8.8-8.6a5.5 5.5 0 0 0 0-7.8Z",
    plus: "M12 5v14M5 12h14",
    close: "m6 6 12 12M18 6 6 18",
    arrow: "M5 12h14m-6-6 6 6-6 6",
    back: "M19 12H5m6-6-6 6 6 6",
    chevron: "m9 5 7 7-7 7",
    down: "m5 9 7 7 7-7",
    queue: "M4 5h16M4 12h11M4 19h11m4-5 4 3-4 3",
    shuffle: "M3 5h3l12 14h3m-4-4 4 4-4 3M3 19h3l4-5m4-4 4-5h3m-4-3 4 3-4 4",
    repeat:
      "m17 2 4 4-4 4M3 11V9a3 3 0 0 1 3-3h15M7 22l-4-4 4-4m14-1v2a3 3 0 0 1-3 3H3",
    volume: "M11 4 5 9H2v6h3l6 5V4Zm4 4a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14",
    mute: "M11 4 5 9H2v6h3l6 5V4Zm5 5 6 6m0-6-6 6",
    expand: "M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5",
    collapse: "M3 8h5V3m13 5h-5V3M8 21v-5H3m13 5v-5h5",
    mini: "M3 4h18v16H3V4Zm9 9h9v7h-9v-7Z",
    eq: "M5 3v8m0 4v6M12 3v3m0 4v11M19 3v12m0 4v2M2 11h6M9 6h6m1 9h6",
    wave: "M2 10v4m4-8v12m4-15v18m4-16v14m4-11v8m4-6v4",
    bass: "M2 12h4l3-7 6 14 3-7h4",
    link: "m10 13 4-4m-6 7-1 1a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m2 1 1-1a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0",
    grip: "M8 5h.01M16 5h.01M8 12h.01M16 12h.01M8 19h.01M16 19h.01",
    trash: "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7",
    pin: "m8 3 8 0-1 7 4 3H5l4-3-1-7Zm4 10v9",
    minimize: "M5 16h14",
    maximize: "M4 4h16v16H4V4Z",
  };
</script>

<svg
  data-icon={name}
  use:reveal={{ key: name, initial: false, duration: 180, y: 0, scale: 0.82 }}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={name === "play" ? "currentColor" : "none"}
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"><path d={paths[name] || paths.wave} /></svg
>
