---
id: PRA-SOUND-PRACTICES
class: canon
tier: 2
scope: [practices]
owner: sound-lead
reviewed: 2026-11-20
disciplines: [sound]
---

<!--
An example practice of the gamedev preset (docs/features/department-practices.md).
Copy this file into your project under docs/spec/practices/sound/, or propose
it there with `spec propose create`.
Declare its prefix in [ids]: PRA = { kind = "practice", shape = "name" }.
The rules below are examples: replace them with your department's own.
-->

# Sound: agreed rules

The sound department's agreed rules for every sound that goes into the game. A task card of the `sound` discipline carries them as its practices; cite a rule by its ID when a sound keeps or departs from it.

## Loudness target {#PRA-SOUND-LOUDNESS}

The game's mix targets -16 LUFS integrated, measured over a representative minute of play, with true peaks below -1 dBTP. Dialogue sits on top of the mix; music and ambience leave it room.

## Formats {#PRA-SOUND-FORMATS}

Sources are kept as 48 kHz, 24-bit WAV files in the sound folder; the engine's import settings make the shipped formats. Short effects load whole, music and long ambience stream.

## Naming {#PRA-SOUND-NAMING}

A sound file is named `<category>_<object>_<action>_<variant>`, in lower case with underscores, the variant a two-digit number, so the engine's events find their variations by name.
