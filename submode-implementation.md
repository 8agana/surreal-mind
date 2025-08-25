# Submode Implementation Plan

## Cognitive Framework Implementation

### Core Architecture
- CognitiveEngine with weighted framework blending
- Frameworks: OODA, Socratic, FirstPrinciples, RootCause, Lateral
- Each returns FrameworkOutput with insights, questions, next_steps

### Submode Blending Weights
- **Sarcastic**: 60% OODA, 30% Socratic, 10% Lateral
- **Philosophical**: 40% FirstPrinciples, 40% Socratic, 20% Lateral  
- **Empathetic**: 50% RootCause, 30% Socratic, 20% FirstPrinciples
- **Problem_solving**: 40% OODA, 30% RootCause, 30% FirstPrinciples

### Still Needed
1. Add schema fields: framework_enhanced (bool), submode (string)
2. Implement SystemsThinking and DialecticalThinking frameworks
3. Connect framework outputs to thought enrichment
4. Store framework analysis results with thoughts
5. Proportional representation for weighted blending (take 6 from OODA if 60%, 3 from Socratic if 30%, etc.)

### Integration Point
The convo_think tool should pass submode → CognitiveEngine → get blended framework analysis → enrich thought content before storage.

## Submode Downstream Effects

### Memory Injection Scaling
- **Sarcastic**: Lower injection threshold, favors contrarian memories
- **Philosophical**: Higher depth search, abstract concept linking
- **Empathetic**: Prioritizes emotional context memories
- **Problem_solving**: Focuses on solution-pattern memories

### Orbital Mechanics Adjustments
- **Sarcastic**: Tighter orbits for recent contradictions
- **Philosophical**: Wider orbits for conceptual connections
- **Empathetic**: Emotion-weighted gravity calculations
- **Problem_solving**: Solution-similarity affects orbital distance

### Thought Enrichment
- **Sarcastic**: Adds wit markers, irony detection
- **Philosophical**: Adds metaphysical connections, abstract links
- **Empathetic**: Adds emotional resonance scoring
- **Problem_solving**: Adds solution pathway mapping

### Bidirectional Memory Effects
- Different submodes create different memory "flavors"
- Future recalls weight memories by matching submode
- Cross-submode memories get transformation functions
- Submode consistency affects memory trust scores

### Relevance Scoring
- Each submode has different relevance algorithms
- Affects which memories get pulled into working set
- Changes similarity threshold requirements
- Modifies vector distance calculations for retrieval

## Implementation Status
- Framework blending foundation built
- Downstream effects designed but not implemented
- Schema updates pending
- Integration with convo_think pending