/-
  Elisp.Eval — Reference evaluator for the core subset.

  Uses `partial def` to avoid termination obligations. This is fine for
  a differential-testing oracle. To enable proofs later, convert to a
  fuel-indexed or small-step version.
-/

import Elisp.Ast
import Elisp.Env

namespace Elisp

/-- Evaluation errors. -/
inductive EvalError where
  | unboundVariable   : (name : Sym) → EvalError
  | unboundFunction   : (name : Sym) → EvalError
  | wrongNumberOfArgs : (expected got : Nat) → EvalError
  | wrongTypeArgument : (msg : String) → EvalError
  | thrown            : (tag value : Val) → EvalError
  | conditionCaseError : (msg : String) → EvalError
  | internalError     : (msg : String) → EvalError
  deriving Repr, Inhabited

abbrev EvalResult := Except EvalError (Env × Val)

/-- Parse a cons-chain into a flat list. -/
def toList : Val → List Val
  | .cons a b => a :: toList b
  | .nil      => []
  | v         => [v]

/-- Coerce a list of Vals into integers, erroring on non-integers.
    Arithmetic primitives are integer-only in the oracle subset. -/
def asInts : List Val → Except EvalError (List Int)
  | []              => .ok []
  | .int n :: rest  => do let ns ← asInts rest; .ok (n :: ns)
  | _ :: _          => .error (.wrongTypeArgument "number")

/-- Variadic addition: (+) = 0, folds with (·+·). -/
def primAdd (vs : List Val) : Except EvalError Val := do
  let ns ← asInts vs
  .ok (.int (ns.foldl (· + ·) 0))

/-- Variadic subtraction. (- n) = -n; (- a b c) = a - b - c. Errors on 0 args. -/
def primSub (vs : List Val) : Except EvalError Val := do
  let ns ← asInts vs
  match ns with
  | []             => .error (.wrongNumberOfArgs 1 0)
  | [n]            => .ok (.int (-n))
  | first :: rest  => .ok (.int (rest.foldl (· - ·) first))

/-- Variadic multiplication: (*) = 1. -/
def primMul (vs : List Val) : Except EvalError Val := do
  let ns ← asInts vs
  .ok (.int (ns.foldl (· * ·) 1))

/-- Variadic division. (/ n) = 1/n. Any zero divisor signals an error.

    Uses `Int.tdiv` (truncation toward zero) to match Rust's `i64 / i64`
    and Emacs's `/`. The default `/` on `Int` in Lean 4 is Euclidean,
    which disagrees on negative divisors (e.g. (-1) / (-2) = 1 vs. 0). -/
def primDiv (vs : List Val) : Except EvalError Val := do
  let ns ← asInts vs
  match ns with
  | []            => .error (.wrongNumberOfArgs 1 0)
  | [n]           =>
    if n == 0 then .error (.wrongTypeArgument "division by zero")
    else .ok (.int (Int.tdiv 1 n))
  | first :: rest =>
    if rest.any (· == 0) then .error (.wrongTypeArgument "division by zero")
    else .ok (.int (rest.foldl Int.tdiv first))

/-- Numeric equality: t iff all args are equal. Matches Emacs Lisp
    semantics — 0 or 1 arg is vacuously t. -/
def primNumEq (vs : List Val) : Except EvalError Val := do
  let ns ← asInts vs
  match ns with
  | []           => .ok .t
  | n :: rest    => .ok (if rest.all (· == n) then .t else .nil)

/-- Pairwise chain comparison helper for ordering primitives. Matches
    Emacs semantics — 0 or 1 arg is vacuously t (no adjacent pair violates
    the order), mirroring the Rust fixnum fast path in primitives_value.rs. -/
def chainCmp (cmp : Int → Int → Bool) (vs : List Val)
    : Except EvalError Val := do
  let ns ← asInts vs
  if ns.length < 2 then .ok .t
  else
    let pairs := ns.zip ns.tail
    .ok (if pairs.all (fun p => cmp p.1 p.2) then .t else .nil)

def primLt (vs : List Val) : Except EvalError Val :=
  chainCmp (fun a b => decide (a < b)) vs
def primGt (vs : List Val) : Except EvalError Val :=
  chainCmp (fun a b => decide (a > b)) vs
def primLe (vs : List Val) : Except EvalError Val :=
  chainCmp (fun a b => decide (a ≤ b)) vs
def primGe (vs : List Val) : Except EvalError Val :=
  chainCmp (fun a b => decide (a ≥ b)) vs

/-- Emacs `/=`: t iff every adjacent pair differs. 0 or 1 arg is
    vacuously t (matches the Rust fixnum fast path). -/
def primNe (vs : List Val) : Except EvalError Val := do
  let ns ← asInts vs
  let pairs := ns.zip ns.tail
  .ok (if pairs.all (fun p => p.1 != p.2) then .t else .nil)

/-- Parse binding forms: ((x 1) (y 2)) → [(x, valExpr)] -/
def parseBindings : List Val → List (Sym × Val)
  | [] => []
  | .cons (.sym s) (.cons v .nil) :: rest => (s, v) :: parseBindings rest
  | .sym s :: rest => (s, .nil) :: parseBindings rest
  | _ :: rest => parseBindings rest

/-- Flatten a list of frames into a single frame. -/
def flattenFrames (frames : List Frame) : Frame :=
  frames.foldl (· ++ ·) []

/-- Parse lambda parameter list into (positional, rest). -/
def parseLambdaParams : List Val → List Sym → Bool
    → Except EvalError (List Sym × Option Sym)
  | [], acc, _                    => .ok (acc.reverse, none)
  | .sym "&rest" :: rest, acc, _  => parseLambdaParams rest acc true
  | .sym s :: _, acc, true        => .ok (acc.reverse, some s)
  | .sym s :: rest, acc, false    => parseLambdaParams rest (s :: acc) false
  | _, _, _                       => .error (.wrongTypeArgument "symbol expected in lambda list")

/-- Bind positional params to args, returning (frame, remaining_args). -/
def bindParams : List Sym → List Val → Frame → Frame × List Val
  | [], remaining, acc       => (acc.reverse, remaining)
  | p :: ps, v :: as_, acc   => bindParams ps as_ ((p, v) :: acc)
  | p :: ps, [], acc         => bindParams ps [] ((p, .nil) :: acc)

/-- `car` — first element of a cons cell. -/
def primCar : List Val → Except EvalError Val
  | [v] => do
    match v with
    | .cons a _ => .ok a
    | .nil      => .ok .nil
    | _         => .error (.wrongTypeArgument " cons or list")
  | vs => .error (.wrongNumberOfArgs 1 vs.length)

/-- `cdr` — rest of a cons cell. -/
def primCdr : List Val → Except EvalError Val
  | [v] => do
    match v with
    | .cons _ d => .ok d
    | .nil      => .ok .nil
    | _         => .error (.wrongTypeArgument " cons or list")
  | vs => .error (.wrongNumberOfArgs 1 vs.length)

/-- `cons` — construct a new cons cell. -/
def primCons : List Val → Except EvalError Val
  | [a, d] => .ok (.cons a d)
  | vs     => .error (.wrongNumberOfArgs 2 vs.length)

/-- `list*` — construct a list with the last arg as the cdr. -/
def primListStar : List Val → Except EvalError Val
  | []     => .error (.wrongNumberOfArgs 1 0)
  | [d]    => .ok d
  | [a, d]  => .ok (.cons a d)
  | a :: rest => do
    let tail ← primListStar rest
    .ok (.cons a tail)

/-- The core evaluator. All helpers are defined via `where` to enable
    mutual recursion under a single `partial` annotation. -/
partial def eval (env : Env) (expr : Val) : EvalResult :=
  match expr with
  | .nil       => .ok (env, .nil)
  | .t         => .ok (env, .t)
  | .int n     => .ok (env, .int n)
  | .str s     => .ok (env, .str s)
  | .lam p r b e => .ok (env, .lam p r b e)
  | .sym s     =>
    match env.lookup s with
    | some v => .ok (env, v)
    | none   => .error (.unboundVariable s)
  | .cons car cdr =>
    let args := toList cdr
    match car with
    | .sym "quote" =>
      match args with
      | [v] => .ok (env, v)
      | _   => .error (.wrongNumberOfArgs 1 args.length)
    | .sym "if"              => evalIf env args
    | .sym "progn"           => evalProgn env args
    | .sym "setq"            => evalSetq env args
    | .sym "let"             => evalLet env args
    | .sym "let*"            => evalLetStar env args
    | .sym "dlet"            => evalDlet env args
    | .sym "lambda"          => evalLambda env args
    | .sym "defun"           => evalDefun env args
    | .sym "catch"           => evalCatch env args
    | .sym "throw"           => evalThrow env args
    | .sym "unwind-protect"  => evalUnwindProtect env args
    | .sym "condition-case"  => evalConditionCase env args
    | .sym "defvar"          => evalDefvar env args
    | .sym "funcall"         => evalFuncall env args
    | .sym "eval"            => evalEval_ env args
    | .sym "+"               => evalPrim env args primAdd
    | .sym "-"               => evalPrim env args primSub
    | .sym "*"               => evalPrim env args primMul
    | .sym "/"               => evalPrim env args primDiv
    | .sym "="               => evalPrim env args primNumEq
    | .sym "<"               => evalPrim env args primLt
    | .sym ">"               => evalPrim env args primGt
    | .sym "<="              => evalPrim env args primLe
    | .sym ">="              => evalPrim env args primGe
    | .sym "/="              => evalPrim env args primNe
    | .sym "list"            => evalList env args
    | .sym "mapcar"          => evalMapcar env args
    | .sym "cond"            => evalCond env args
    | .sym "and"             => evalAnd_ env args
    | .sym "or"              => evalOr_ env args
    | .sym "when"            => evalWhen_ env args
    | .sym "unless"          => evalUnless_ env args
    | .sym "prog1"           => evalProg1 env args
    | .sym "prog2"           => evalProg2 env args
    | .sym "car"             => evalPrim env args primCar
    | .sym "cdr"             => evalPrim env args primCdr
    | .sym "cons"            => evalPrim env args primCons
    | .sym "list*"           => evalPrim env args primListStar
    | _                      => evalCall env car args
where
  evalIf (env : Env) (args : List Val) : EvalResult :=
    match args with
    | cond :: then_ :: else_ => do
      let (env', condVal) ← eval env cond
      if condVal != .nil then eval env' then_
      else evalProgn env' else_
    | [cond] => do
      let (env', condVal) ← eval env cond
      if condVal != .nil then .error (.wrongNumberOfArgs 2 1)
      else .ok (env', .nil)
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalProgn (env : Env) (forms : List Val) : EvalResult :=
    match forms with
    | []        => .ok (env, .nil)
    | [f]       => eval env f
    | f :: rest => do
      let (env', _) ← eval env f
      evalProgn env' rest

  evalSetq (env : Env) (args : List Val) : EvalResult :=
    match args with
    | []  => .ok (env, .nil)
    | [_] => .error (.wrongTypeArgument "odd number of args to setq")
    | .sym s :: valExpr :: rest => do
      let (env', v) ← eval env valExpr
      let env'' := env'.setVar s v
      if rest.isEmpty then .ok (env'', v)
      else evalSetq env'' rest
    | _ => .error (.wrongTypeArgument "symbol expected in setq")

  evalLet (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bindsVal :: body =>
      let bindSpecs := parseBindings (toList bindsVal)
      evalLetBinds env bindSpecs [] body
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalLetBinds (env : Env) (specs : List (Sym × Val)) (acc : Frame)
      (body : List Val) : EvalResult :=
    match specs with
    | [] =>
      let innerEnv := { env with lex := env.lex.push acc }
      evalProgn innerEnv body
    | (s, valExpr) :: rest => do
      let (env', v) ← eval env valExpr
      evalLetBinds env' rest (acc ++ [(s, v)]) body

  evalLetStar (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bindsVal :: body =>
      let bindSpecs := parseBindings (toList bindsVal)
      evalLetStarGo env bindSpecs body
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalLetStarGo (env : Env) (specs : List (Sym × Val))
      (body : List Val) : EvalResult :=
    match specs with
    | [] => evalProgn env body
    | (s, valExpr) :: rest => do
      let (env', v) ← eval env valExpr
      evalLetStarGo { env' with lex := env'.lex.push [(s, v)] } rest body

  evalDlet (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bindsVal :: body =>
      let bindSpecs := parseBindings (toList bindsVal)
      let savedDyn := env.dyn
      evalDletGo env bindSpecs savedDyn body
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalDletGo (env : Env) (specs : List (Sym × Val)) (savedDyn : DynEnv)
      (body : List Val) : EvalResult :=
    match specs with
    | [] => do
      let (env', v) ← evalProgn env body
      .ok ({ env' with dyn := savedDyn }, v)
    | (s, valExpr) :: rest => do
      let (env', v) ← eval env valExpr
      evalDletGo { env' with dyn := env'.dyn.push [(s, v)] } rest savedDyn body

  evalLambda (env : Env) (args : List Val) : EvalResult :=
    match args with
    | paramsVal :: body =>
      let paramList := toList paramsVal
      match parseLambdaParams paramList [] false with
      | .error e => .error e
      | .ok (params, rest) =>
        let captured := flattenFrames env.lex.frames
        .ok (env, .lam params rest body captured)
    | _ => .error (.wrongNumberOfArgs 1 args.length)

  evalDefun (env : Env) (args : List Val) : EvalResult :=
    match args with
    | .sym name :: paramsVal :: body => do
      let (env', lamVal) ← evalLambda env (paramsVal :: body)
      let env'' := { env' with functions := (name, lamVal) :: env'.functions }
      .ok (env'', .sym name)
    | _ => .error (.wrongTypeArgument "defun requires a symbol name")

  evalCatch (env : Env) (args : List Val) : EvalResult :=
    match args with
    | tagExpr :: body => do
      let (env', tag) ← eval env tagExpr
      match evalProgn env' body with
      | .ok result => .ok result
      | .error (.thrown thrownTag value) =>
        if thrownTag == tag then .ok (env', value)
        else .error (.thrown thrownTag value)
      | .error e => .error e
    | _ => .error (.wrongNumberOfArgs 1 args.length)

  evalThrow (env : Env) (args : List Val) : EvalResult :=
    match args with
    | [tagExpr, valExpr] => do
      let (env', tag) ← eval env tagExpr
      let (_, value) ← eval env' valExpr
      .error (.thrown tag value)
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalUnwindProtect (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bodyForm :: cleanup =>
      match eval env bodyForm with
      | .ok (env', v) => do
        let (env'', _) ← evalProgn env' cleanup
        .ok (env'', v)
      | .error e =>
        let _ := evalProgn env cleanup
        .error e
    | _ => .error (.wrongNumberOfArgs 1 args.length)

  /-- `condition-case` — established error handlers around a body form.
      Syntax: (condition-case BODYFORM HANDLERS...)
      where each HANDLER is (ERROR-SYMBOL BODY...).
      For the oracle subset, we match thrown errors by symbol name. -/
  evalConditionCase (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bodyForm :: [] => eval env bodyForm
    | bodyForm :: handlers =>
      match eval env bodyForm with
      | .ok (env', v) => .ok (env', v)
      | .error (.thrown thrownTag value) =>
        runConditionCaseHandler env thrownTag value handlers
      | .error e => .error e
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  /-- Run the first matching handler for a thrown error. -/
  runConditionCaseHandler (env : Env) (tag : Val) (value : Val)
      (handlers : List Val) : EvalResult :=
    match handlers with
    | [] => .error (.thrown tag value)
    | handler :: rest =>
      match handler with
      | .cons (.sym handlerTag) body =>
        if handlerTag == ".error" then
          evalProgn env (toList body)
        else
          match tag with
          | .sym s =>
            if handlerTag == s then
              evalProgn env (toList body)
            else
              runConditionCaseHandler env tag value rest
          | _ => runConditionCaseHandler env tag value rest
      | _ => runConditionCaseHandler env tag value rest

  /-- `defvar SYMBOL VALUE` — declare a dynamic variable.
      If SYMBOL is not already bound as a dynamic variable, create a
      dynamic binding with VALUE. If already bound, leave the existing
      binding untouched ( Emacs semantics). Always returns the symbol. -/
  evalDefvar (env : Env) (args : List Val) : EvalResult :=
    match args with
    | [.sym s, valExpr] => do
      let (env', v) ← eval env valExpr
      if env'.dynVars.contains s then
        .ok (env', .sym s)
      else
        .ok ({ env' with
                dynVars := s :: env'.dynVars,
                dyn := env'.dyn.push [(s, v)] }, .sym s)
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  /-- `funcall FN ARG...` — call FN with evaluated arguments.
      Unlike a bare call where the head is a symbol looked up in the
      function slot, `funcall` evaluates FN first (so any callable
      value works: a lambda, a symbol-as-function, etc.). -/
  evalFuncall (env : Env) (args : List Val) : EvalResult :=
    match args with
    | []      => .error (.wrongNumberOfArgs 1 0)
    | fnExpr :: rest => do
      let (env', fnVal) ← eval env fnExpr
      let (env'', evaledArgs) ← evalCallArgs env' rest []
      applyFn env'' fnVal evaledArgs

  /-- `eval FORM` — evaluate FORM at runtime (re-entrant evaluation).
      This is the same tree-walker as the top-level `eval`; the oracle
      is a pure interpreter so re-entrant invocation has no semantic
      cost beyond normal recursion. -/
  evalEval_ (env : Env) (args : List Val) : EvalResult :=
    match args with
    | [form] => do
      let (_, v) ← eval env form
      .ok (env, v)
    | _ => .error (.wrongNumberOfArgs 1 args.length)

  evalPrim (env : Env) (args : List Val)
      (f : List Val → Except EvalError Val) : EvalResult := do
    let (env', vs) ← evalCallArgs env args []
    match f vs with
    | .ok v    => .ok (env', v)
    | .error e => .error e

  /-- Dispatch a call. Emacs Lisp is a Lisp-2: a symbol head is looked up
      in the function slot (populated by `defun`), not the variable slot.
      We fall back to the variable slot so that a symbol bound via `let` to
      a lambda still works (mimicking `funcall`), and to the dispatch already
      in `applyFn` for unresolved symbols. -/
  evalCall (env : Env) (fnExpr : Val) (args : List Val) : EvalResult := do
    match fnExpr with
    | .sym s =>
      let (env', evaledArgs) ← evalCallArgs env args []
      match (env'.lookupFn s).orElse fun _ => env'.lookup s with
      | some fnVal => applyFn env' fnVal evaledArgs
      | none       => .error (.unboundFunction s)
    | _ => do
      let (env',  fnVal)      ← eval env fnExpr
      let (env'', evaledArgs) ← evalCallArgs env' args []
      applyFn env'' fnVal evaledArgs

  evalCallArgs (env : Env) (remaining : List Val) (acc : List Val)
      : Except EvalError (Env × List Val) :=
    match remaining with
    | [] => .ok (env, acc.reverse)
    | arg :: rest => do
      let (env', v) ← eval env arg
      evalCallArgs env' rest (v :: acc)

  applyFn (env : Env) (fn : Val) (args : List Val) : EvalResult :=
    match fn with
    | .lam params rest body captured =>
      let (frame, remaining) := bindParams params args []
      let frame' := match rest with
        | some restSym =>
          let restList := remaining.foldr (fun v a => Val.cons v a) .nil
          frame ++ [(restSym, restList)]
        | none => frame
      let closureEnv : LexEnv := { frames := [captured] }
      let callEnv := { env with lex := closureEnv.push frame' }
      match evalProgn callEnv body with
      | .ok (_, result) => .ok (env, result)
      | .error e => .error e
    | .sym s =>
      match env.lookupFn s with
      | some fnVal => applyFn env fnVal args
      | none => .error (.unboundFunction s)
    | _ => .error (.wrongTypeArgument s!"not a function: {repr fn}")

  /-- `list` primitive: evaluate all args, return a proper cons-list terminated
      by nil. -/
  evalList (env : Env) (args : List Val) : EvalResult := do
    let (env', vs) ← evalCallArgs env args []
    let result := vs.foldr (fun v acc => Val.cons v acc) .nil
    .ok (env', result)

  /-- `mapcar FN LIST`: apply FN to each element of LIST and collect results
      into a new list. The function value can be any callable — a symbol
      (looked up in function slot), a `lambda`/`closure` value, or similar. -/
  evalMapcar (env : Env) (args : List Val) : EvalResult :=
    match args with
    | [fnExpr, listExpr] => do
      let (env',  fnVal)   ← eval env fnExpr
      let (env'', listVal) ← eval env' listExpr
      let items := toList listVal
      mapcarGo env'' fnVal items []
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  mapcarGo (env : Env) (fnVal : Val) (items : List Val) (acc : List Val)
      : EvalResult :=
    match items with
    | [] =>
      let result := acc.reverse.foldr (fun v a => Val.cons v a) .nil
      .ok (env, result)
    | item :: rest => do
      let (env', r) ← applyFn env fnVal [item]
      mapcarGo env' fnVal rest (r :: acc)

  evalCond (env : Env) (clauses : List Val) : EvalResult :=
    match clauses with
    | [] => .ok (env, .nil)
    | clause :: rest =>
      match clause with
      | .cons testExpr thenExprs => do
        let (env', testVal) ← eval env testExpr
        if testVal != .nil then
          if thenExprs == .nil then .ok (env', testVal)
          else evalProgn env' (toList thenExprs)
        else evalCond env' rest
      | _ => do
        let (env', testVal) ← eval env clause
        if testVal != .nil then .ok (env', testVal)
        else evalCond env' rest

  evalAnd_ (env : Env) (args : List Val) : EvalResult :=
    match args with
    | []      => .ok (env, .t)
    | [e]     => eval env e
    | e :: es => do
      let (env', v) ← eval env e
      if v == .nil then .ok (env', .nil)
      else evalAnd_ env' es

  evalOr_ (env : Env) (args : List Val) : EvalResult :=
    match args with
    | []      => .ok (env, .nil)
    | [e]     => eval env e
    | e :: es => do
      let (env', v) ← eval env e
      if v != .nil then .ok (env', v)
      else evalOr_ env' es

  evalWhen_ (env : Env) (args : List Val) : EvalResult :=
    match args with
    | cond :: body => do
      let (env', v) ← eval env cond
      if v != .nil then evalProgn env' body
      else .ok (env', .nil)
    | _ => .error (.wrongNumberOfArgs 2 0)

  evalUnless_ (env : Env) (args : List Val) : EvalResult :=
    match args with
    | cond :: body => do
      let (env', v) ← eval env cond
      if v == .nil then evalProgn env' body
      else .ok (env', .nil)
    | _ => .error (.wrongNumberOfArgs 2 0)

  evalProg1 (env : Env) (args : List Val) : EvalResult :=
    match args with
    | []      => .error (.wrongNumberOfArgs 1 0)
    | [e]     => eval env e
    | e :: es => do
      let (env', v) ← eval env e
      let _ ← evalProgn env' es
      .ok (env', v)

  evalProg2 (env : Env) (args : List Val) : EvalResult :=
    match args with
    | []        => .error (.wrongNumberOfArgs 2 0)
    | [_e1]      => .error (.wrongNumberOfArgs 2 1)
    | [_e1, e2]  => do
      let (env', _) ← eval env _e1
      eval env' e2
    | _e1 :: e2 :: es => do
      let (env', _) ← eval env _e1
      let (env'', v) ← eval env' e2
      let _ ← evalProgn env'' es
      .ok (env'', v)

end Elisp
