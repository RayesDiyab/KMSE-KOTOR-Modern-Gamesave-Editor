# Timeline art

The pictures the KOTOR Modern Gamesave Editor's galaxy map draws (the Timeline tab; until
2026-11-06 the timeline, which the galaxy map replaced, hence the folder's name).

| File | What | Source |
| --- | --- | --- |
| `ebon-hawk.png` | The Ebon Hawk, the button for its side chapter | A render from Deadly Stream, free to use (the maintainer, 2026-10-24). Cut from its background with rembg's BiRefNet model; the engines' glow and haze, which the cut took for background, put back from the original's warm pixels. 400 px wide |
| `yavin-station-mask.png` | The outline of Yavin Station | Not the picture: the station is a frame of KotOR's own movie (Live_1c.bik, frame 484), so `build.rs` makes it from the player's game with ffmpeg at build time and only this outline is kept here (made once with rembg's BiRefNet model from that frame enlarged four times). Without the game or ffmpeg the button shows its name alone |
| `stars.png` | The starfield behind the galaxy map | Generated (about 2,600 stars and two soft glows), 1800 by 675, 128 colours |
