# SkiFree — multiplatform port (personal project)

A from-scratch, multiplatform reimplementation of **SkiFree** (`ski32.exe`, the 2005 32-bit recompile of Chris Pirih's 1991 classic), based on a full reverse engineering of the original binary with [REA](https://github.com/morluto/rea) + Ghidra.

**Goal:** a faithful, playable version of the game on modern platforms (macOS first), for personal use.

## Repository layout

| Path | Content |
|---|---|
| `ski32.exe` / `ski32.zip` | Original 32-bit binary (target of the analysis) |
| `SKI.EXE` / `ski.zip` | Original 16-bit Windows 3.x version (reference) |
| `docs/how-ski32-works.md` | How the binary works: game loop, input, physics, rendering, sound, persistence |
| `docs/monster-ai.md` | The yeti AI: spawn guards, rubber-band chase, eating sequence |
| `TODO.md` | Progress tracker for the port |

## Key findings so far

- Plain Win32 GDI game: one window, one 40 ms timer (ID 666, 25 fps), a linked list of ≤100 actors, 89 bitmap sprites, 9 WAV sounds, high scores in `entpack.ini`
- The famous 2000 m yeti is one of **four** boundary-guard monsters (top/left/right/bottom of the course); it chases by rubber-banding to the player's position every tick, which is why it can't be outrun
- Cheat keys survive in the 32-bit build: `F` fast-forward, `T` slow-motion step, `X`/`Y` teleport

## Notes

- Pseudocode references in the docs come from Ghidra 12.2 decompiler output, not original source
- Original game © Chris Pirih / Microsoft. This repo is private and for personal use; extracted assets are not for redistribution
