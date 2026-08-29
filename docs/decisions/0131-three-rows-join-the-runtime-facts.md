# 0131 — Three rows join the runtime facts, and nothing else

Date: 2026-08-30, with unit 37.

## Context

The runtime-facts tool answers what the assistant is and where it runs. Asked to gain
OS information, the tool could grow any number of host facts: kernel version, hostname,
memory, load. Each one sounds harmless on its own and the list has no natural end.

## Decision

Exactly three rows join: the distribution, the processor architecture, and the public
homes of the software. These are the facts that were asked for, and they are the facts
a member asking "what do you run on" is actually asking about.

They join the runtime-facts tool itself. That tool renders its answer as rows and takes
another one without restructuring, and unit 34 already settled that a newly named fact
about the running process belongs there and not in a second tool beside it.

## Rejected alternatives

- **A separate os-info tool.** A second tool answering the same question splits the
  answer in two: a model asking one of them gets a partial picture, the palette grows a
  name that means almost what the other one means, and the two descriptions start
  competing for the same question.
- **The kernel version, the hostname, the memory or the load.** Nobody asked, and each
  is either a deployment detail that belongs in no chat answer or a number that changes
  between the read and the reply.
