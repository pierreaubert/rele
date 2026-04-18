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

/-- Replace the binding for `s` in the first lexical frame that contains it.
    Returns `none` if no frame contains `s`. This mirrors the Rust
    interpreter's behaviour where `setq` writes back into the existing
    binding rather than creating a new shadowing one. -/
def updateLexFrame (s : Sym) (v : Val) : List Frame → Option (List Frame)
  | []         => none
  | f :: rest  =>
    if f.any (fun (n, _) => n == s) then
      let f' := f.map fun (n, old) => if n == s then (n, v) else (n, old)
      some (f' :: rest)
    else
      (updateLexFrame s v rest).map (f :: ·)

/-- Set a variable. Dynamic-declared vars go to dynEnv; otherwise we mutate
    the innermost lexical frame that already binds `s`, falling back to
    creating a new dynamic binding if no lexical frame owns `s`. -/
def setVar (env : Env) (s : Sym) (v : Val) : Env :=
  if env.dynVars.contains s then
    { env with dyn := env.dyn.push [(s, v)] }
  else
    match updateLexFrame s v env.lex.frames with
    | some newFrames => { env with lex := { frames := newFrames } }
    | none           => { env with dyn := env.dyn.push [(s, v)] }

end Env

end Elisp
