The regeneration system runs after movement:

```rust
fn regenerate(mut query: Query<(&mut Stamina, &Resting)>, time: Res<Time>) {
    for (mut stamina, resting) in &mut query {
        if resting.for_secs >= 1.5 {
            stamina.value = (stamina.value + 10.0 * time.delta_secs()).min(stamina.max);
        }
    }
}
```
