/-
  Elisp.Env — Lexical and dynamic environment model.

  Emacs Lisp has two binding disciplines:
  - Lexical: bindings are resolved by static scope (closures capture their env).
  - Dynamic: bindings are resolved by the call stack (specpdl in real Emacs).

  The oracle models both: `let` creates lexical bindings (when
  lexical-binding is t), `dlet` always creates dynamic bindings,
  `defvar`'d symbols are always dynamic.
-/

import Elisp.Ast

namespace Elisp

/-- A single environment frame (association list). -/
abbrev Frame := List (Sym × Val)

/-- Lexical environment: a stack of frames. Inner frames shadow outer. -/
structure LexEnv where
  frames : List Frame := []
  deriving Repr

/-- Dynamic environment: a flat association list (most-recent-first).
    Mirrors Emacs's specpdl: push on bind, pop on unbind. -/
structure DynEnv where
  bindings : List (Sym × Val) := []
  deriving Repr

/-- Combined interpreter state. -/
structure Env where
  lex       : LexEnv  := {}
  dyn       : DynEnv  := {}
  /-- Symbols declared with `defvar` / `defconst` — always use dynamic binding. -/
  dynVars   : List Sym := []
  /-- Global function table (defun'd functions). -/
  functions : List (Sym × Val) := []
  deriving Repr

namespace LexEnv

def lookup (env : LexEnv) (s : Sym) : Option Val :=
  env.frames.findSome? fun frame =>
    (frame.find? fun (name, _) => name == s).map Prod.snd

def push (env : LexEnv) (frame : Frame) : LexEnv :=
  { frames := frame :: env.frames }

end LexEnv

namespace DynEnv

def lookup (env : DynEnv) (s : Sym) : Option Val :=
  (env.bindings.find? fun (name, _) => name == s).map Prod.snd

def push (env : DynEnv) (bindings : Frame) : DynEnv :=
  { bindings := bindings ++ env.bindings }

/-- Restore dynamic env to a previous state (for unwind-protect). -/
def restore (_env : DynEnv) (saved : DynEnv) : DynEnv := saved

end DynEnv

namespace Env

/-- Look up a symbol: dynamic-declared vars use dynEnv, others use lexEnv,
    falling back to dynEnv for free variables (Emacs compat). -/
def lookup (env : Env) (s : Sym) : Option Val :=
  if env.dynVars.contains s then
    env.dyn.lookup s
  else
    (env.lex.lookup s).orElse fun _ => env.dyn.lookup s

/-- Look up a function by name. -/
def lookupFn (env : Env) (s : Sym) : Option Val :=
  (env.functions.find? fun (name, _) => name == s).map Prod.snd

/-- Set a variable. Dynamic-declared vars go to dynEnv;
    others mutate the innermost lexical frame (or fall back to dynamic). -/
def setVar (env : Env) (s : Sym) (v : Val) : Env :=
  -- Simplified: push onto dynamic for both cases.
  -- A full implementation would mutate the lexical frame in-place.
  { env with dyn := { bindings := (s, v) :: env.dyn.bindings } }

end Env

end Elisp
