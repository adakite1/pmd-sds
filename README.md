This is a non-retail-matching decompilation of Pokemon Mystery Dungeon: Explorers of Sky, written in Rustlang using LLVM as the compiler suite, meant to run in parallel with pret/pmd-sky in the spirit of a mountain hike of two, where a piece of assembly is interpreted twice by the two projects for a boost in accuracy, more about that later.

The criteria for when an assembly function is considered done are necessarily different because of the codegen. Each assembly function is considered done once it has the same side-effects and pure calculations (terms from functional programming; I think this describes it precisely) by comparison with the original by eye. More roughly, they should do the exact same things as what the original function does to the outside world including RAM, VRAM, registers, game state, and have the same behaviors for outputs given the same inputs for pure calculations, but they are allowed to do so differently internally (signed vs unsigned loops, jumping-to-a-branch vs branch tables, etc.).

There has been substantial work done already, including finishing crt0. Read on for the purpose and several choices made.

(This is as yet an unannounced personal project of mine, so you would be one of the early people to have found this. Congrats! Before you join, please read [CONTRIBUTING.md](./CONTRIBUTING.md) carefully, as culture is very important for us.) [![Join our Discord](https://img.shields.io/badge/Discord-5865F2?logo=discord&logoColor=white)](https://discord.gg/JzkcMJ9nmR)

### Purpose

As we've already established, this is a decompilation of PMD: EoS, though there are a few things that would benefit from further specification.

First, this decompilation is from scratch, meaning functionality is matched by comparing the assembly of each symbol. We also try to do this from scratch, including for symbols where pmd-sky might already have a matching decompilation written in C, the reasons for which are detailed later, down below. The included tooling builds the asm-corpus from a clone of the pmd-sky repo (by convention pinned at commit f14ee2c which happened to be the HEAD at the start of the project and has been kept at it for stability of symbol labeling), utilizing the wonderful work that has already been done by the pmd-sky contributors splitting the game's code into respective function ranges, and this collection of assembly files is the baseline the decompilation starts from and the progress it tracks; asm-corpus, whenever it is mentioned, is just every single byte in the ROM's code, laid out as text files.

Why? Well, there's the personal reason, which is that I am both a bare-metal programmer and also a Rustlang programmer, and have been at it for many years. However there have been very few decompilations attempted in Rustlang as far as I can tell, and yet I do not see a technical reason why a non-matching decompilation could not be done with the language. As for why though, it also allows for several opportunities to try some things differently from how they are usually done:

- First change: *Starting with crt0 and working outward following the call tree.*

I'm betting that this could speed up work, since every single symbol done provides immediate context to the next, including init and setup.

This means though that we can't skip crt0, boot code, or the SDK. We also work in a tree-branch fashion, where we hit symbols as we trace.

- Second change, and I mentioned this part above already, but I want to elaborate here: *Putting the focus on functional equivalency.*

Decompilation does not have to happen in one pass. This project is never and will never be matching since LLVM codegen differs massively from Metrowerks, but this has an unexpected boon of forcing a different perspective:

Since it becomes possible to focus entirely on logical unity of groups of symbols, whether something is correct or not can be determined by the standard of whether the resulting decompilation makes sense as a whole in a group, which is a different way to look at correctness. For instance, one can drill on whether a constant actually means what it is called and work on making any interdependence on a specific system across the game more explicit by paying for it with non-matching parts.

Or, think of code that runs on both CPUs. Since different codegen also immediately forces shifts in addresses from day one, it becomes easy to immediately disentangle contingent addresses from their weakest preconditions for placement ("this value just has to be in fast memory, so DTCM or WRAM"). This is true beyond dual-CPU code as well, functions can be examined for what side effects they need to cause and what calculations they need to make, separated from what they might happen to do. There were already some SDK sync functions that had this difference between the actual implementation and the reasonably inferrable intent of the programmer, and the nature of the project asks this question for every symbol.

This is a good place to lead into a point I promised to talk about above, which is why we are intentionally starting from scratch in terms of decompilation, as well as what this project means in relation to pmd-sky as well:

#### I think of this project as a second pair of eyes on the same opaque assembly.

This is why both projects should continue in parallel, which is my firm belief, and also why I think having a system like this can be extremely valuable in and of itself.

For instance, let's say two projects looking at the same assembly came to different conclusions, then that would be an immediate gold-mine of questions and interesting discussions ready to be dug into. And then if we both came to the same conclusions independently, then the chances of both having gotten it wrong are very small, and we can be all the more confident.

In other words, the different approach gives us potentially differing assumptions we are making, differing ways of first encountering the same symbols, as well as different mental models. I think it could be somewhat fun too, because, well... more people is always more fun anyways I think.

Low-level hardware programming is something that I have always loved doing, and this is a very tasty project I'm excited about; I hope that I can pull in more people along as well who might be great at low-level hardware work and great at using Rustlang for it. If I can do so, then I am already satisfied. The more sharp-eyed and passionate people we have here anywhere, working on something which we all think is valuable and interesting, the merrier, no matter how we go about it.

### Contributing

Culture is something I take very seriously, and so for everyone who wants to join us (thank you!), make sure to read the following document: [CONTRIBUTING.md](./CONTRIBUTING.md). Otherwise, we'll see you there! I'm looking forward to talking about the nth weirdly named SDK symbol with you.

