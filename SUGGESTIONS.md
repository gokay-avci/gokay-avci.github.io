# Content & Messaging Suggestions

## Overview
This document outlines specific suggestions to improve the clarity, consistency, and impact of the website's content.

## Homepage (`templates/index.html`)

1.  **Hero Copy:**
    *   **Current:** "Computational materials scientist building reproducible, HPC-ready pipelines for porous materials, and robust scientific tooling in Rust/WASM."
    *   **Suggestion:** "Computational materials scientist specializing in reproducible HPC workflows, porous materials simulation, and high-performance scientific tooling in Rust/WASM."
    *   **Reason:** "Specializing in" flows better than "building" when listing multiple areas.

2.  **Sub-Text / Pillars:**
    *   **Current:** "Porous materials • host–guest chemistry • workflow orchestration • simulation→ML acceleration • general lifestyle blogging"
    *   **Suggestion:** "Porous Materials • Host–Guest Chemistry • Workflow Orchestration • Simulation→ML Acceleration • Personal Journal"
    *   **Reason:** Matches the navigation label ("Journal") and sounds more professional.

3.  **Typos:**
    *   "If you just want too see some demos" -> "If you just want to see some demos"

## About Page (`content/about.md`)

1.  **Typo Fixes:**
    *   "you should feel like home1!" -> "at home!"
    *   "chivalirous" -> "chivalrous"
    *   "Motive implementation" -> "Motivate implementation"
    *   "belive" -> "believe"

2.  **Phrasing Improvements:**
    *   **Rust Section:** "I eventually was brought the presence of a programming language named Rust." -> "I eventually discovered Rust." or "I eventually transitioned to Rust."
    *   **Intro:** "My doctoral work was on... yet my interests have evolved..." -> Simplify to focus on current expertise while acknowledging the background.

3.  **Contact:**
    *   Make the email address clickable: `<a href="mailto:gokayavcichem@gmail.com">gokayavcichem@gmail.com</a>` (or markdown syntax `[email](mailto:...)`)

## Project Descriptions

1.  **Structure:** Consider a consistent format for project descriptions (e.g., Problem, Solution, Technologies Used).
2.  **Detail:** Expand on the impact of each project. For example, "HPC Orchestrator" could mention "Reduced simulation setup time by 40%".

## Navigation (`config.toml`)

*   **Labels:** Currently "About", "Journal", "CV", "Projects".
*   **Suggestion:** These are clear and functional. "Journal" adds a personal touch which fits the "Personal Journal" change on the homepage.
