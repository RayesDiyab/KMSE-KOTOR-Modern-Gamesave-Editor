# Timeline art

The pictures the KOTOR Modern Gamesave Editor's timeline draws (Story, Timeline).

| File | What | Source |
| --- | --- | --- |
| `ebon-hawk.png` | The Ebon Hawk, the button for its side chapter | A render from Deadly Stream, free to use (the maintainer, 2026-10-24). Cut from its background with rembg's BiRefNet model; the engines' glow and haze, which the cut took for background, put back from the original's warm pixels. 400 px wide |
| `yavin-station-mask.png` | The outline of Yavin Station | Not the picture: the station is a frame of KotOR's own movie (Live_1c.bik, frame 484), so `build.rs` makes it from the player's game with ffmpeg at build time and only this outline is kept here (made once with rembg's BiRefNet model from that frame enlarged four times). Without the game or ffmpeg the button shows its name alone |
| `stars.png` | The starfield behind the timeline | Generated (about 2,600 stars and two soft glows), 1800 by 675, 128 colours |
