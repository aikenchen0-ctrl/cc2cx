export interface WorkflowPreset {
  id: string
  name: string
  description: string
  /** File entries: relative filename -> content */
  files: Record<string, string>
}

export const WORKFLOW_PRESETS: WorkflowPreset[] = [
  {
    id: 'commit',
    name: 'Smart Commit',
    description: 'Generate conventional commit message from staged changes',
    files: {
      'commit.md': [
        'Analyze staged changes (git diff --cached) and generate a conventional commit message.',
        'Follow the Conventional Commits specification (feat/fix/docs/style/refactor/test/chore).',
        'Include scope when clear from the changed files.',
        'Keep the subject line under 72 characters.',
        'Add a body if the changes are non-trivial.',
      ].join('\n'),
    },
  },
  {
    id: 'review',
    name: 'Code Review',
    description: 'Review current changes for bugs, style, and best practices',
    files: {
      'review.md': [
        'Review the current uncommitted changes (git diff) for:',
        '1. Potential bugs or logic errors',
        '2. Code style and consistency issues',
        '3. Performance concerns',
        '4. Security vulnerabilities',
        '5. Missing error handling',
        'Provide actionable feedback with file:line references.',
      ].join('\n'),
    },
  },
  {
    id: 'test-gen',
    name: 'Test Generator',
    description: 'Generate unit tests for the current file or function',
    files: {
      'test-gen.md': [
        'Generate comprehensive unit tests for the specified file or function.',
        'Use the project\'s existing test framework and patterns.',
        'Include edge cases, error paths, and boundary conditions.',
        'Follow AAA (Arrange-Act-Assert) pattern.',
      ].join('\n'),
    },
  },
  {
    id: 'refactor',
    name: 'Refactor Assistant',
    description: 'Suggest and apply refactoring improvements',
    files: {
      'refactor.md': [
        'Analyze the specified code for refactoring opportunities:',
        '- Extract methods/functions for repeated logic (DRY)',
        '- Simplify complex conditionals (KISS)',
        '- Ensure single responsibility (SRP)',
        '- Reduce coupling and improve cohesion',
        'Apply changes incrementally and explain each refactoring.',
      ].join('\n'),
    },
  },
  {
    id: 'explain',
    name: 'Code Explainer',
    description: 'Explain how a piece of code works in detail',
    files: {
      'explain.md': [
        'Explain the specified code in detail:',
        '- What it does at a high level',
        '- How each major section works',
        '- Key algorithms or patterns used',
        '- Dependencies and side effects',
        'Use clear language appropriate for the code\'s complexity.',
      ].join('\n'),
    },
  },
]
