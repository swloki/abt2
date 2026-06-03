# Hyperscript Reference Guide

Source: https://hyperscript.org/reference/ & https://hyperscript.org/patterns/

---

## Features (Top-Level Constructs)

Every hyperscript program is one or more features placed in the `_`, `script`, or `data-script` attribute.

| Feature | Description | Example |
|---------|-------------|---------|
| `on` | Event listener | `on click log "clicked!"` |
| `def` | Define a function | `def foo() log "bar" end` |
| `init` | Run on first load | `init set x to 0` |
| `set` | Element-scoped variable | `set x to 0` |
| `behavior` | Reusable cross-cutting behavior | `behavior Draggable ... end` |
| `install` | Install a behavior | `install Draggable` |
| `js` | Embed JS at top level | `js alert("hi"); end` |
| `live` | Reactive commands, re-runs on dependency change | `live set $total to ($price * $qty)` |
| `when` | React to value changes | `when $x changes ...` |
| `bind` | Two-way sync between values | `bind .dark and #toggle's checked` |

### Extension Features
- **components** — Custom elements with reactive templates
- **eventsource** — Server-Sent Events (SSE)
- **socket** — WebSocket
- **worker** — Web Worker

---

## `on` Feature (Detailed)

```
on [every | first] <event-spec>
   (or [every | first] <event-spec>)*
   [queue (all | first | last | none)]
   <command>+
[end]
```

### Event Spec
```
<event-name> [(<param-list>)] [[<filter>]] [<count>]
              [from <expr> | from elsewhere | elsewhere]
              [in <expr>]
              [debounced at <time> | throttled at <time>]
```

- **every** — no synchronization, all events fire handler in parallel
- **first** — fires only once
- **queue none** — drop events while handler active
- **queue all** — queue all events in order
- **queue first** — queue only the first event
- **queue last** — queue only the last event (DEFAULT)
- **from elsewhere** — listen for events from outside the element (click-away)
- **from <expr>** — listen on another element
- **in <expr>** — scope to subtree (e.g. `on click in .menu-item ...`)
- **debounced at 500ms** — wait until 500ms of silence
- **throttled at 500ms** — fire at most every 500ms
- **or** — combine multiple events: `on click or touchstart ...`
- **count** — `on click 1` (once), `on click 2 to 10`, `on click 11 and on`

### Synthetic Events
- **mutation** — `on mutation of @foo ...` (Mutation Observer)
- **intersection** — `on intersection(intersecting) having threshold 0.5 ...`
- **resize** — `on resize(width, height) ...`

### Exception Handling
```
on click call mightThrow()
on exception(error) log error
```

---

## Commands

| Command | Description | Example |
|---------|-------------|---------|
| `add` | Add class/content | `add .myClass to me` |
| `remove` | Remove class/element | `remove .myClass from me` / `remove me` |
| `toggle` | Toggle class | `toggle .clicked on me` |
| `take` | Move class from siblings | `take .active from .tabs` |
| `show` | Show element | `show #div` |
| `hide` | Hide element | `hide me` |
| `put ... into` | Set value/innerHTML | `put "hello" into me` |
| `set ... to` | Set variable | `set x to 0` |
| `get` | Evaluate expression into `it` | `get my value` |
| `call` | Call JS function | `call alert('hi')` |
| `fetch` | HTTP fetch | `fetch /api as JSON` |
| `send` / `trigger` | Dispatch event | `send customEvent to #el` |
| `wait` | Wait for time/event | `wait 2s` / `wait for customEvent` |
| `transition` | Animate CSS property | `transition *opacity to 0` |
| `settle` | Wait for transition end | `add .fade then settle` |
| `if` / `else` / `else if` | Conditional | `if x > 0 log x end` |
| `repeat` | Loop | `repeat for x in arr log x end` |
| `repeat 3 times` | Counted loop | `repeat 3 times ... end` |
| `break` | Break loop | `break` |
| `continue` | Skip loop iteration | `continue` |
| `return` | Return value | `return 42` |
| `exit` | Exit handler | `if x is null exit` |
| `throw` | Throw exception | `throw "Bad Value"` |
| `log` | Console log | `log me` |
| `increment` | x += 1 | `increment counter` |
| `decrement` | x -= 1 | `decrement counter` |
| `default ... to` | Set if undefined | `default x to 0` |
| `append ... to` | Append to string/array | `append "val" to arr` |
| `make` | Create instance/element | `make a <p/> called para` |
| `measure` | Get element measurements | `measure me then log it` |
| `morph` | DOM morph | `morph #target to content` |
| `render` | Render template | `render #tpl with items: data` |
| `go` | Navigate | `go to /about` / `go back` |
| `scroll` | Scroll | `scroll to #section smoothly` |
| `focus` | Focus element | `focus #input` |
| `blur` | Unfocus element | `blur me` |
| `open` | Open dialog/fullscreen | `open #dialog` / `open fullscreen` |
| `close` | Close dialog/fullscreen | `close #dialog` |
| `select` | Select text in input | `select #search` |
| `reset` | Reset form | `reset #my-form` |
| `empty` / `clear` | Clear content | `empty #results` |
| `tell` | Set implicit target | `tell <p/> remove yourself` |
| `swap` | Swap two values | `swap x with y` |
| `ask` | Browser prompt | `ask "Your name?"` |
| `answer` | Alert/confirm | `answer "Save?" with "Yes" or "No"` |
| `js` | Embed JS block | `js alert('hi'); end` |
| `beep!` | Debug print | `beep! <.foo/>` |
| `pick` | Select from collection | `pick first 3 of arr` |
| `halt` | Stop event propagation | `halt` |
| `speak` | Text-to-speech | `speak "Hello"` |
| `breakpoint` | DevTools pause | `breakpoint` |

---

## Expressions

### DOM References
| Syntax | Meaning | Example |
|--------|---------|---------|
| `#id` | Element by ID | `#main-div` |
| `.class` | Class reference | `.active` |
| `<selector/>` | CSS query selector | `<button/>`, `<:focused/>` |
| `@attr` | Attribute reference | `@data-foo` |
| `*style` | Style reference | `*color`, `*computed-fontSize` |
| `closest <sel/>` | Closest ancestor | `closest <div/>` |
| `next <sel/>` / `previous <sel/>` | Relative navigation | `next <div/>` |
| `first from <sel/>` | Positional | `first from <div/>` |

### Property Access
| Syntax | Meaning | Example |
|--------|---------|---------|
| `.prop` | Dot notation | `event.target` |
| `'s` / `of` | Possessive | `the window's location` / `the location of window` |
| `[n]` | Index | `items[0]` |

### Operators
| Syntax | Meaning | Example |
|--------|---------|---------|
| `+ - * /` | Math | `x + 1` |
| `mod` | Modulo (not `%`) | `x mod 3` |
| `is` / `is not` | Equality (`==`/`!=`) | `x is "foo"` |
| `is really` | Strict equality (`===`) | `x is really "foo"` |
| `matches` | CSS selector / regex match | `I match .active` |
| `contains` / `includes` | Contains | `arr contains "foo"` |
| `is in` | Reverse contains | `"foo" is in arr` |
| `starts with` / `ends with` | String prefix/suffix | `url starts with "https"` |
| `is between X and Y` | Inclusive range | `x is between 1 and 10` |
| `precedes` / `follows` | DOM order | `#a precedes #b` |
| `exists` | Not null, non-empty collection | `<.results/> exists` |
| `is empty` / `is not empty` | Emptiness check | `x is empty` |
| `is a` / `is an` | Type check | `x is a String` |
| `no` | Emptiness/negation | `no element.children` |
| `some` | Existence | `some <.results/>` |
| `and` / `or` / `not` | Logical | `x and y` |
| `as Type` | Type conversion | `"10" as Int` |
| `as Values \| JSONString` | Pipe conversion chain | `x as Values \| JSONString` |
| `ignoring case` | Case-insensitive modifier | `x is "admin" ignoring case` |

### Collection Expressions
| Syntax | Meaning | Example |
|--------|---------|---------|
| `where` | Filter | `items where its active` |
| `sorted by` | Sort | `items sorted by its name descending` |
| `mapped to` | Map | `items mapped to its id` |
| `split by` | Split string | `"a,b" split by ","` |
| `joined by` | Join array | `items joined by ", "` |

### Literals
| Syntax | Meaning | Example |
|--------|---------|---------|
| `"..."` / `'...'` | String (supports `${}`) | `"Hello ${name}"` |
| `200ms` / `2s` | Time expression | `wait 200ms` |
| `10px` / `2em` / `50%` | CSS units | `scroll down by 200px` |
| `\ x -> x * x` | Block literal (lambda) | `\ x -> x * x` |
| `[1,2,3]` | Array | `[1, 2, 3]` |
| `{foo:"bar"}` | Object | `{foo: "bar"}` |
| `true` / `false` / `null` | Boolean/null | `true` |

---

## Magic Values

| Value | Description | Example |
|-------|-------------|---------|
| `me` / `my` / `I` | Current element | `put "hello" into me` |
| `you` / `your` / `yourself` | Target from `tell` | `tell <p/> remove yourself` |
| `it` / `its` / `result` | Previous command result | `fetch /api then put it into me` |
| `event` | Current event object | `log event.type` |
| `target` | `event.target` | `add .clicked to target` |
| `detail` | `event.detail` | `log detail.message` |
| `sender` | `event.detail.sender` | `log sender.id` |
| `body` | `document.body` | `put "hi" into body` |
| `cookies` | Cookie API | `cookies['My-Cookie']` |
| `clipboard` | System clipboard | `put clipboard into me` |
| `selection` | Selected text | `put selection into #out` |

---

## Lifecycle Events

| Event | When |
|-------|------|
| `hyperscript:ready` | After hyperscript processes page |
| `load` | After element's hyperscript loads |
| `hyperscript:before:init` | Before element init |
| `hyperscript:after:init` | After element init |
| `hyperscript:before:cleanup` | Before element cleanup |
| `hyperscript:after:cleanup` | After element cleanup |
| `exception` | Runtime error (detail.error) |

---

## Common Patterns

### Toggle Class
```html
<button _="on click toggle .active">Toggle</button>
```

### Fade & Remove
```html
<div _="on click transition *opacity to 0 then remove me">Fade out</div>
```

### Tabs
```html
<button _="on click take .active from .tabs">Tab 1</button>
```

### Click-Outside Close
```html
<div _="on click elsewhere close me">
  Click outside to close
</div>
```

### Fetch & Update
```html
<div _="on click fetch /api then put it into me">Load</div>
```

### Debounced Input
```html
<input _="on keyup debounced at 300ms fetch /search?q=${my value} then put it into #results">
```

### Keyboard Shortcut
```html
<div _="on keydown[key=='Escape'] from elsewhere hide me">
```

### Multiple Events
```html
<div _="on click or touchstart doSomething()">...</div>
```

### Filter + Search
```html
<input _="on keyup show <.item/> when its textContent contains my value">
```

### Async Transparency (no await needed)
```html
<div _="on click fetch /api wait 2s log it">
  <!-- fetch and wait are both async but no await/callback needed -->
</div>
```

### Reactive with `live`
```html
<div _="live set $total to ($price * $qty)">
  Total: <span _="bind $total to my textContent"></span>
</div>
```

### Modal Dialog
```html
<dialog id="modal">
  Content
  <button _="on click close #modal">Close</button>
</dialog>
<button _="on click open #modal">Open</button>
```

### Event Queue Strategy
```html
<!-- Only process the last click while handler is running -->
<button _="on click queue last fetch /api then put it into me">Load</button>

<!-- Drop all events while handler is running -->
<button _="on click queue none fetch /api then put it into me">Load</button>
```

### Mutation Observer
```html
<div _='on mutation of @data-status log "changed!"'>Watch attribute</div>
```

### Intersection Observer
```html
<img _="on intersection(intersecting) having threshold 0.5
         if intersecting transition *opacity to 1
         else transition *opacity to 0"
     src="...">
```
