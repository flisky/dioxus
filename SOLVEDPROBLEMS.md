# Solved problems while building Dioxus

focuses:
- ergonomics
- render agnostic
- remote coupling
- memory efficient
- concurrent 
- global context
- scheduled updates
- 


## FC Macro for more elegant components
Originally the syntax of the FC macro was meant to look like:

```rust
#[fc]
fn example(ctx: &Context<{ name: String }>) -> VNode {
    html! { <div> "Hello, {name}!" </div> }
}
```

`Context` was originally meant to be more obviously parameterized around a struct definition. However, while this works with rustc, this does not work well with Rust Analyzer. Instead, the new form was chosen which works with Rust Analyzer and happens to be more ergonomic. 

```rust
#[fc]
fn example(ctx: &Context, name: String) -> VNode {
    html! { <div> "Hello, {name}!" </div> }
}
```

## Anonymous Components

In Yew, the function_component macro turns a struct into a Trait `impl` with associated type `props`. Like so:

```rust
#[derive(Properties)]
struct Props {
    // some props
}

struct SomeComponent;
impl FunctionProvider for SomeComponent {
    type TProps = Props;

    fn run(&mut self, props: &Props) -> Html {
        // user's functional component goes here
    }
}

pub type SomeComponent = FunctionComponent<function_name>;
```
By default, the underlying component is defined as a "functional" implementation of the `Component` trait with all the lifecycle methods. In Dioxus, we don't allow components as structs, and instead take a "hooks-only" approach. However, we still need props. To get these without dealing with traits, we just assume functional components are modules. This lets the macros assume an FC is a module, and `FC::Props` is its props and `FC::component` is the component. Yew's method does a similar thing, but with associated types on traits.

Perhaps one day we might use traits instead.

The FC macro needs to work like this to generate a final module signature:

```rust
// "Example" can be used directly
// The "associated types" are just children of the module
// That way, files can just be components (yay, no naming craziness)
mod Example {
    // Associated metadata important for liveview
    static NAME: &'static str = "Example";

    struct Props {
        name: String
    }
    
    fn component(ctx: &Context<Props>) -> VNode {
        html! { <div> "Hello, {name}!" </div> }
    }
}

// or, Example.rs

static NAME: &'static str = "Example";

struct Props {
    name: String
}

fn component(ctx: &Context<Props>) -> VNode {
    html! { <div> "Hello, {name}!" </div> }
}
```

These definitions might be ugly, but the fc macro cleans it all up. The fc macro also allows some configuration

```rust
#[fc]
fn example(ctx: &Context, name: String) -> VNode {
    html! { <div> "Hello, {name}!" </div> }
}

// .. expands to 

mod Example {
    use super::*;
    static NAME: &'static str = "Example";
    struct Props {
        name: String
    }    
    fn component(ctx: &Context<Props>) -> VNode {
        html! { <div> "Hello, {name}!" </div> }
    }
}
```



## Live Components
Live components are a very important part of the Dioxus ecosystem. However, the goal with live components was to constrain their implementation purely to APIs available through Context (concurrency, context, subscription). 

From a certain perspective, live components are simply server-side-rendered components that update when their props change. Here's more-or-less how live components work:

```rust
#[fc]
static LiveFc: FC = |ctx, refresh_handler: impl FnOnce| {
    // Grab the "live context"
    let live_context = ctx.use_context::<LiveContext>();

    // Ensure this component is registered as "live"
    live_context.register_scope();

    // send our props to the live context and get back a future
    let vnodes = live_context.request_update(ctx);

    // Suspend the rendering of this component until the vnodes are finished arriving
    // Render them once available
    ctx.suspend(async move {
        let output = vnodes.await;

        // inject any listener handles (ie button clicks, views, etc) to the parsed nodes
        output[1].add_listener("onclick", refresh_handler);

        // Return these nodes
        // Nodes skip diffing and go straight to rendering
        output
    })
}
```

Notice that LiveComponent receivers (the client-side interpretation of a LiveComponent) are simply suspended components waiting for updates from the LiveContext (the context that wraps the app to make it "live"). 

## Allocation Strategy (ie incorporating Dodrio research)
----
The `VNodeTree` type is a very special type that allows VNodes to be created using a pluggable allocator. The html! macro creates something that looks like:

```rust
static Example: FC<()> = |ctx| {
    html! { <div> "blah" </div> }
};

// expands to...

static Example: FC<()> = |ctx| {
    // This function converts a Fn(allocator) -> VNode closure to a DomTree struct that will later be evaluated.
    html_macro_to_vnodetree(move |allocator| {
        let mut node0 = allocator.alloc(VElement::div);
        let node1 = allocator.alloc_text("blah");
        node0.children = [node1];
        node0
    })
};
```
At runtime, the new closure is created that captures references to `ctx`. Therefore, this closure can only be evaluated while `ctx` is borrowed and in scope. However, this closure can only be evaluated with an `allocator`. Currently, the global and Bumpalo allocators are available, though in the future we will add support for creating a VDom with any allocator or arena system (IE Jemalloc, wee-alloc, etc). The intention here is to allow arena allocation of VNodes (no need to box nested VNodes). Between diffing phases, the arena will be overwritten as old nodes are replaced with new nodes. This saves allocation time and enables bump allocators.



## Context and lifetimes

We want components to be able to fearlessly "use_context" for use in state management solutions.

However, we cannot provide these guarantees without compromising the references. If a context mutates, it cannot lend out references.

Functionally, this can be solved with UnsafeCell and runtime dynamics. Essentially, if a context mutates, then any affected components would need to be updated, even if they themselves aren't updated. Otherwise, a reference would be pointing at data that could have potentially been moved. 

To do this safely is a pretty big challenge. We need to provide a method of sharing data that is safe, ergonomic, and that fits the abstraction model.

Enter, the "ContextGuard".

The "ContextGuard" is very similar to a Ref/RefMut from the RefCell implementation, but one that derefs into actual underlying value. 

However, derefs of the ContextGuard are a bit more sophisticated than the Ref model. 

For RefCell, when a Ref is taken, the RefCell is now "locked." This means you cannot take another `borrow_mut` instance while the Ref is still active. For our purposes, our modification phase is very limited, so we can make more assumptions about what is safe.

1. We can pass out ContextGuards from any use of use_context. These don't actually lock the value until used.
2. The ContextGuards only lock the data while the component is executing and when a callback is running.
3. Modifications of the underlying context occur after a component is rendered and after the event has been run.
   
With the knowledge that usage of ContextGuard can only be achieved in a component context and the above assumptions, we can design a guard that prevents any poor usage but also is ergonomic.

As such, the design of the ContextGuard must:
- be /copy/ for easy moves into closures
- never point to invalid data (no dereferencing of raw pointers after movable data has been changed (IE a vec has been resized))
- not allow references of underlying data to leak into closures

To solve this, we can be clever with lifetimes to ensure that any data is protected, but context is still ergonomic.

1. As such, deref context guard returns an element with a lifetime bound to the borrow of the guard. 
2. Because we cannot return locally borrowed data AND we consume context, this borrow cannot be moved into a closure. 
3. ContextGuard is *copy* so the guard itself can be moved into closures
4. ContextGuard derefs with its unique lifetime *inside* closures
5. Derefing a ContextGuard evaluates the underlying selector to ensure safe temporary access to underlying data

```rust
struct ExampleContext {
    // unpinnable objects with dynamic sizing
    items: Vec<String>
}

fn Example<'src>(ctx: Context<'src, ()>) -> VNode<'src> {
    let val: &'b ContextGuard<ExampleContext> = (&'b ctx).use_context(|context: &'other ExampleContext| {
        // always select the last element
        context.items.last()
    });

    let handler1 = move |_| println!("Value is {}", val); // deref coercion performed here for printing
    let handler2 = move |_| println!("Value is {}", val); // deref coercion performed here for printing

    ctx.view(html! {
        <div>
            <button onclick={handler1}> "Echo value with h1" </button>
            <button onclick={handler2}> "Echo value with h2" </button>
            <div>
                <p> "Value is: {val}" </p>
            </div>
        </div>
    })
}
```

A few notes:
- this does *not* protect you from data races!!!
- this does *not* force rendering of components
- this *does* protect you from invalid + UB use of data
- this approach leaves lots of room for fancy state management libraries
- this approach is fairly quick, especially if borrows can be cached during usage phases


## Concurrency

I don't even know yet