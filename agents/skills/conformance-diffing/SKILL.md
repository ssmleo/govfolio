# skill: conformance-diffing
Purpose: read and extend adapter conformance
Load when: verifying adapters, bouncing builds
Core checklist:
- run conformance bin -> read unified diff -> classify: parser bug vs fixture wrong vs regime change -> route accordingly
Anti-patterns: editing expected.json to make red green without human
Write-back: deepen this file when the procedure teaches you something; same PR.
