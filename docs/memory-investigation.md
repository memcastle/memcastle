# Memory investigation (2026-10-11)

The original daemon held approximately 11.38 GiB of private swap when sampled, with approximately 90 MiB resident
and an earlier 8 GiB resident high-water mark.
Those figures establish a large historical footprint, but do not identify its allocation site.

The installed service was inactive during this follow-up; its warning-level journal did not record successful jobs or
requests.
The installed configuration did not enable extraction or embeddings, so neither sweep explains this run.
Its embedded palace occupied approximately 5 GiB on disk and contained 22,541 drawers at the time of the follow-up.
Measurements used a temporary copy of that palace and a checkout-built 0.4.1 daemon under a 1,200 MiB cgroup memory
limit with no cgroup swap; the installed service and palace were not started or changed.

On the copy, idle RSS stayed near 170 MiB for a minute and 100 status requests settled near 264 MiB RSS.
Fifty sequential `/api/wings` requests grew from approximately 166 MiB at idle to 439 MiB RSS, then levelled off;
the first request took 1.46 seconds.
This route is expensive at this scale, but that run did not reproduce continuing multi-gigabyte growth.

The audit job revealed a specific avoidable allocation: it called `store.list_drawers(None)`, which fetched every
drawer's full content and embedding into one response and then deserialized every row, although the audit only needed
the ID, room, provenance job ID and presence of an embedding.
On the copied palace, two sequential audits raised RSS from 168 MiB before the jobs to a peak of 865 MiB, with
approximately 836 MiB still resident ten seconds later (no swap).
After changing the audit and repair orphan scan to read narrow pages of 256 drawers, two audits on the same copy rose
from 253 MiB before the jobs to a peak and settled RSS of 445 MiB (no swap).
The different starting baselines reflect the embedded engine's cache and process warm-up; the measured incremental
growth was approximately 668 MiB before and 192 MiB after the change.
Both audits completed in each run, and targeted audit, repair and pagination tests passed.

This establishes an audit/repair memory improvement, not that an audit caused all 11 GiB of the former daemon's swap.
The old daemon's successful-job timeline is unavailable, and a fresh copied palace does not replay its full history or
memory pressure from other processes.
If large growth recurs, record timestamped process RSS, private anonymous PSS, swap, cgroup memory and swap alongside
job kinds, request counts and host pressure; an allocation profile during the actual growing workload is the next step.
