# Timeline art

The pictures the KOTOR Modern Gamesave Editor's galaxy map draws (the Timeline tab; until
2026-11-06 the timeline, which the galaxy map replaced, hence the folder's name).

| File | What | Source |
| --- | --- | --- |
| `ebon-hawk.png` | The Ebon Hawk, the button for its side chapter | A render from Deadly Stream, free to use (the maintainer, 2026-10-24). Cut from its background with rembg's BiRefNet model; the engines' glow and haze, which the cut took for background, put back from the original's warm pixels. 400 px wide |
| `endar-spire.png` | The Endar Spire, the first stop of KotOR's route | A render on black supplied by the maintainer on 2026-11-06, for the map; its origin is not recorded here. Cut from its background by brightness: every dark pixel reachable from the picture's edge through dark pixels is background, the ship's edge pixels take an alpha from their brightness and their colour back from the black they were drawn over; cropped to the ship and area-averaged to 400 px wide (400 by 325) |
| `leviathan.png` | The Leviathan, KotOR's stop after the third star map | A render supplied by the maintainer on 2026-11-06 (1920 by 1080, the ship over a planet among fighters); its origin is not recorded here. Cut with BiRefNet (the model rembg calls birefnet-general, run with onnxruntime from a copy already on the PC), only the largest shape kept, its colour taken 80% towards grey and lightened by a fifth (the maintainer: "make it a bit more grey its too blue"), area-averaged to 400 px wide (400 by 165). The uncut picture was deleted, as the maintainer asked |
| `ravager.png` | The Ravager, KotOR II's stop before Malachor V | A picture supplied by the maintainer on 2026-11-06 (640 by 272, on stars, a second ship beside it); its origin is not recorded here. Cut as the Leviathan, without the colour change (400 by 185). The uncut picture was deleted, as the maintainer asked |
| `yavin-station-mask.png` | The outline of Yavin Station | Not the picture: the station is a frame of KotOR's own movie (Live_1c.bik, frame 484), so `build.rs` makes it from the player's game with ffmpeg at build time and only this outline is kept here (made once with rembg's BiRefNet model from that frame enlarged four times). Without the game or ffmpeg the button shows its name alone |
| `stars.png` | The starfield behind the galaxy map | Generated (about 2,600 stars and two soft glows), 1800 by 675, 128 colours |
