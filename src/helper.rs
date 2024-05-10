#[inline(always)]
pub fn lerp(u: f32, v: f32, t: f32) -> f32 {
    // t * (v - u) + u
    t.mul_add(v-u, u)
}

#[inline(always)]
pub fn inverse_lerp(u: f32, v: f32, lerp: f32) -> f32 {
    (lerp - u) / (v - u)
}

#[inline(always)]
pub fn clamp_up_1f(value: &mut f32, limit: f32) {
    if *value > limit {
        *value = limit;
    }
}

#[inline(always)]
pub fn clamp_up(value: &mut f32) {
    clamp_up_1f(value, 1.0);
}

#[inline(always)]
pub fn clamp_down_1f(value: &mut f32, limit: f32) {
    if *value < limit {
        *value = limit;
    }
}

#[inline(always)]
pub fn clamp_down(value: &mut f32) {
    clamp_down_1f(value, 0.0);
}

#[inline(always)]
pub fn clamp(value: &mut f32) {
    *value = value.clamp(0.0, 1.0);
}

#[inline(always)]
pub fn slide_towards(val: &mut f32, goal: f32, incr: f32) {
    if *val > goal {
        *val -= incr;
        clamp_down_1f(val, goal);
    } else if *val < goal {
        *val += incr;
        clamp_up_1f(val, goal);
    }
}
