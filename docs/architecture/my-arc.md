Um so I had the idea for this architecture where currently it's implemented where the UI talks to desktop, desktop talks to orchestration and orchestration talks to providers and domain. But now I'm worried that domain as a name doesn't actually make too much sense.

What is it actually doing? Like, why can it invoke sub-agents but not invoke tools? And tools are invoked at the orchestration layer. As well as that, um the domain should be.

I don't think I'm actually following at all the domain-driven design or like hex arc, really. Like, I have these files, and these files do things, but whenever I ask the AI to read it, they say, Yeah, it's following it, sure, sure, sure, but it doesn't look like it.

Especially the orchestration folder. This is super. There's a bunch of shit going on that doesn't actually say or do anything particularly well? Like, I could look in the back end, but I don't actually know what that means. And I feel like that with all of my all of these, all of this code here, like, none of it actually is set up in a way that makes sense.

So I want to look for like a tried and true method for HexArc to set it up and I also need to go for each one and do refactor, but at this point looking for it, it doesn't, none of it makes sense. Sorry, help me out, yo.

Try to, like, I don't know, draw me die around with something. I need to figure it out. 


Plan
- Rename Domain to Engine
- Define the Adapters for the engine
    - Tools
    - ???
    - MCP 
- Define that orchestration is invoking the engines
    - Manages workflow persistence and the likes
- 