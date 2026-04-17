/-
  Elisp.Ast — Core AST for the reference subset.

  Covers: integers, strings, symbols, nil, t, cons, lambda.
  This is the JSON-exchangeable subset used for differential testing
  against the Rust interpreter (rele-elisp).
-/

namespace Elisp

/-- Symbol names are interned strings in the real implementation;
    here we just use `String` since we don't need O(1) eq. -/
abbrev Sym := String

/-- Core Elisp value. Intentionally small — only the forms needed
    for the oracle subset (lexical/dynamic binding, control flow).

    Lambda fields are inlined to avoid mutual-inductive complications. -/
inductive Val where
  | nil   : Val
  | t     : Val
  | int   : Int → Val
  | str   : String → Val
  | sym   : Sym → Val
  | cons  : Val → Val → Val
  | lam   : (params : List Sym) → (rest : Option Sym)
           → (body : List Val) → (env : List (Sym × Val)) → Val
  deriving Repr, BEq

end Elisp
