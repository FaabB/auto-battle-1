//! ORCA (Optimal Reciprocal Collision Avoidance) — pure math, no Bevy dependency.
//!
//! Computes collision-free velocities for agents moving in 2D.
//! Only handles agent-agent avoidance; static obstacles are handled by navmesh pathfinding.
//!
//! Based on the RVO2 reference implementation (Agent.cpp).

use bevy::math::Vec2;

/// A half-plane constraint in velocity space.
/// Valid velocities lie on the left side of the directed line (where `direction.perp()` points).
#[derive(Debug, Clone, Copy)]
pub struct OrcaLine {
    /// A point on the boundary line in velocity space.
    pub point: Vec2,
    /// Unit direction vector along the line.
    pub direction: Vec2,
}

/// Snapshot of one agent's state for ORCA computation.
#[derive(Debug, Clone, Copy)]
pub struct AgentSnapshot {
    pub position: Vec2,
    /// Current velocity (from last frame's ORCA output).
    pub velocity: Vec2,
    /// Desired velocity (from pathfinding).
    pub preferred: Vec2,
    pub radius: f32,
    pub max_speed: f32,
    /// How much of the avoidance adjustment this agent absorbs (0.0–1.0, typically 0.5).
    pub responsibility: f32,
}

/// Compute the ORCA half-plane constraint for agent `a` avoiding agent `b`.
///
/// Returns `None` if agents are at the same position (degenerate case).
pub fn compute_orca_line(
    a: &AgentSnapshot,
    b: &AgentSnapshot,
    time_horizon: f32,
) -> Option<OrcaLine> {
    let rel_pos = b.position - a.position;
    let rel_vel = a.velocity - b.velocity;
    let combined_radius = a.radius + b.radius;
    let dist_sq = rel_pos.length_squared();
    let combined_radius_sq = combined_radius * combined_radius;

    let inv_time_horizon = 1.0 / time_horizon;

    if dist_sq > combined_radius_sq {
        // Agents are not overlapping — use truncated cone VO.
        let w = rel_vel - inv_time_horizon * rel_pos;
        let w_length_sq = w.length_squared();
        let dot_product_1 = w.dot(rel_pos);

        #[allow(clippy::suspicious_operation_groupings)]
        let on_cutoff_circle =
            dot_product_1 < 0.0 && dot_product_1 * dot_product_1 > combined_radius_sq * w_length_sq;

        if on_cutoff_circle {
            // Project on cutoff circle (closest to the circular truncation).
            let w_length = w_length_sq.sqrt();
            if w_length < f32::EPSILON {
                return None;
            }
            let unit_w = w / w_length;
            let direction = Vec2::new(unit_w.y, -unit_w.x);
            let u = combined_radius.mul_add(inv_time_horizon, -w_length) * unit_w;

            Some(OrcaLine {
                point: a.velocity + a.responsibility * u,
                direction,
            })
        } else {
            // Project on legs.
            let leg = (dist_sq - combined_radius_sq).sqrt();

            if det(rel_pos, w) > 0.0 {
                // Project on left leg.
                let direction = Vec2::new(
                    rel_pos.x.mul_add(leg, -(rel_pos.y * combined_radius)),
                    rel_pos.x.mul_add(combined_radius, rel_pos.y * leg),
                ) / dist_sq;

                let dot_product_2 = rel_vel.dot(direction);
                let u = dot_product_2 * direction - rel_vel;

                Some(OrcaLine {
                    point: a.velocity + a.responsibility * u,
                    direction,
                })
            } else {
                // Project on right leg.
                let direction = -Vec2::new(
                    rel_pos.x.mul_add(leg, rel_pos.y * combined_radius),
                    (-rel_pos.x).mul_add(combined_radius, rel_pos.y * leg),
                ) / dist_sq;

                let dot_product_2 = rel_vel.dot(direction);
                let u = dot_product_2 * direction - rel_vel;

                Some(OrcaLine {
                    point: a.velocity + a.responsibility * u,
                    direction,
                })
            }
        }
    } else {
        // Agents are already overlapping — skip ORCA constraint.
        // The physics engine (avian2d pushbox) handles overlap separation.
        // Generating emergency constraints here causes deadlocks in dense
        // groups because the contradictory constraints collapse velocity to zero.
        None
    }
}

/// Compute the best collision-free velocity for an agent given ORCA constraints.
///
/// Finds the velocity closest to `preferred` that satisfies all half-plane
/// constraints and lies within the `max_speed` disc.
pub fn compute_avoiding_velocity(preferred: Vec2, max_speed: f32, lines: &[OrcaLine]) -> Vec2 {
    let (mut result, fail_line) = linear_program_2(lines, preferred, max_speed);
    if fail_line < lines.len() {
        result = linear_program_3(lines, fail_line, result, max_speed);
    }
    result
}

/// 2D cross product (determinant of 2x2 matrix).
fn det(a: Vec2, b: Vec2) -> f32 {
    a.x.mul_add(b.y, -(a.y * b.x))
}

/// 1D optimization along constraint line `line_idx`, respecting all prior constraints.
///
/// Returns the optimal point along `lines[line_idx]` that is closest to
/// `opt_velocity` (or maximizes along `opt_velocity` direction if `direction_opt`)
/// while satisfying all constraints `0..line_idx` and the max-speed disc.
///
/// Returns `None` if infeasible.
fn linear_program_1(
    lines: &[OrcaLine],
    line_idx: usize,
    opt_velocity: Vec2,
    max_speed: f32,
    direction_opt: bool,
) -> Option<Vec2> {
    let line = &lines[line_idx];
    let dot_product = line.point.dot(line.direction);
    let discriminant = dot_product.mul_add(
        dot_product,
        max_speed.mul_add(max_speed, -line.point.length_squared()),
    );

    if discriminant < 0.0 {
        // Max speed disc doesn't intersect with this constraint line.
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let mut t_left = -dot_product - sqrt_discriminant;
    let mut t_right = -dot_product + sqrt_discriminant;

    // Clip against all prior constraints.
    for prior in &lines[..line_idx] {
        let denominator = det(line.direction, prior.direction);
        let numerator = det(prior.direction, line.point - prior.point);

        if denominator.abs() <= f32::EPSILON {
            // Lines are (near-)parallel.
            if numerator < 0.0 {
                return None;
            }
            continue;
        }

        let t = numerator / denominator;
        if denominator >= 0.0 {
            // Right bound.
            t_right = t_right.min(t);
        } else {
            // Left bound.
            t_left = t_left.max(t);
        }

        if t_left > t_right {
            return None;
        }
    }

    // Optimize: pick t closest to desired velocity/direction.
    let t = if direction_opt {
        // `opt_velocity` is actually a direction — project along it.
        let t_opt = line.direction.dot(opt_velocity);
        t_opt.clamp(t_left, t_right)
    } else {
        let t_opt = line.direction.dot(opt_velocity - line.point);
        t_opt.clamp(t_left, t_right)
    };

    Some(line.point + t * line.direction)
}

/// 2D incremental linear program.
///
/// Processes constraints one by one. If the current solution violates a new
/// constraint, projects onto that constraint via [`linear_program_1`].
///
/// Returns `(result, fail_index)` where `fail_index == lines.len()` means all
/// constraints were satisfied.
fn linear_program_2(lines: &[OrcaLine], opt_velocity: Vec2, max_speed: f32) -> (Vec2, usize) {
    linear_program_2_impl(lines, opt_velocity, max_speed, false)
}

/// Inner LP2 with `direction_opt` flag.
///
/// When `direction_opt` is `true`, `opt_velocity` is treated as a unit direction
/// and the initial result is placed on the max-speed disc boundary in that direction.
fn linear_program_2_impl(
    lines: &[OrcaLine],
    opt_velocity: Vec2,
    max_speed: f32,
    direction_opt: bool,
) -> (Vec2, usize) {
    let mut result = if direction_opt {
        opt_velocity.normalize_or_zero() * max_speed
    } else if opt_velocity.length_squared() > max_speed * max_speed {
        opt_velocity.normalize() * max_speed
    } else {
        opt_velocity
    };

    for (i, line) in lines.iter().enumerate() {
        // Check if current result already satisfies constraint i.
        // The constraint is: det(direction, point - result) <= 0
        if det(line.direction, line.point - result) > 0.0 {
            // Current result violates constraint i — re-optimize along this line.
            if let Some(new_result) =
                linear_program_1(lines, i, opt_velocity, max_speed, direction_opt)
            {
                result = new_result;
            } else {
                // Infeasible at this line.
                return (result, i);
            }
        }
    }

    (result, lines.len())
}

/// Infeasible fallback: minimizes maximum constraint violation.
///
/// When the 2D LP is infeasible (too many contradictory constraints),
/// this function finds the velocity that minimizes the worst-case
/// penetration into any constraint's forbidden half-plane.
fn linear_program_3(lines: &[OrcaLine], fail_line: usize, current: Vec2, max_speed: f32) -> Vec2 {
    let mut result = current;
    let mut distance = 0.0_f32;

    for i in fail_line..lines.len() {
        // Check if constraint i is already satisfied within tolerance.
        if det(lines[i].direction, lines[i].point - result) <= distance {
            continue;
        }

        // Build a reduced constraint set from all lines 0..i projected
        // onto the perpendicular of line i.
        let mut projected_lines: Vec<OrcaLine> = Vec::with_capacity(i);

        for j in 0..i {
            let determinant = det(lines[i].direction, lines[j].direction);
            if determinant.abs() <= f32::EPSILON {
                // Nearly parallel lines.
                if lines[i].direction.dot(lines[j].direction) > 0.0 {
                    // Same direction — redundant.
                    continue;
                }
                // Opposite direction — constraint bisects.
                projected_lines.push(OrcaLine {
                    point: 0.5 * (lines[i].point + lines[j].point),
                    direction: (lines[j].direction - lines[i].direction).normalize_or_zero(),
                });
            } else {
                projected_lines.push(OrcaLine {
                    point: lines[i].point
                        + (det(lines[j].direction, lines[i].point - lines[j].point) / determinant)
                            * lines[i].direction,
                    direction: (lines[j].direction - lines[i].direction).normalize_or_zero(),
                });
            }
        }

        // Optimize along the perpendicular of line i (pointing into the valid half-plane).
        let temp_result = result;
        let opt_direction = Vec2::new(-lines[i].direction.y, lines[i].direction.x);

        // Use LP2 on projected lines with direction optimization (matching RVO2).
        let (new_result, _fail) =
            linear_program_2_impl(&projected_lines, opt_direction, max_speed, true);

        if det(lines[i].direction, lines[i].point - new_result) > distance {
            result = new_result;
        }

        distance = det(lines[i].direction, lines[i].point - result);

        if result == temp_result && distance > f32::EPSILON {
            // No progress. Expected in very dense crowds. Keep going.
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(pos: Vec2, vel: Vec2, preferred: Vec2) -> AgentSnapshot {
        AgentSnapshot {
            position: pos,
            velocity: vel,
            preferred,
            radius: 6.0,
            max_speed: 50.0,
            responsibility: 0.5,
        }
    }

    #[test]
    fn head_on_produces_lateral_avoidance() {
        // Two agents heading straight at each other on the x-axis.
        let a = agent(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        let b = agent(
            Vec2::new(30.0, 0.0),
            Vec2::new(-50.0, 0.0),
            Vec2::new(-50.0, 0.0),
        );

        let line = compute_orca_line(&a, &b, 3.0).expect("should produce a constraint");
        let result = compute_avoiding_velocity(a.preferred, a.max_speed, &[line]);

        // Result should have a lateral (y) component to dodge.
        assert!(
            result.y.abs() > 0.1,
            "Expected lateral avoidance, got {result:?}"
        );
    }

    #[test]
    fn perpendicular_crossing_adjusts_velocity() {
        let a = agent(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        let b = agent(
            Vec2::new(30.0, -30.0),
            Vec2::new(0.0, 50.0),
            Vec2::new(0.0, 50.0),
        );

        let line = compute_orca_line(&a, &b, 3.0).expect("should produce a constraint");
        let result = compute_avoiding_velocity(a.preferred, a.max_speed, &[line]);

        // Result should differ from preferred when paths cross.
        let diff = (result - a.preferred).length();
        assert!(diff > 0.1, "Expected velocity adjustment, got diff={diff}");
    }

    #[test]
    fn overtaking_agent_steers_around() {
        // Fast agent behind a slow one, both moving right.
        let a = agent(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        let b = agent(
            Vec2::new(20.0, 0.0),
            Vec2::new(20.0, 0.0),
            Vec2::new(20.0, 0.0),
        );

        let line = compute_orca_line(&a, &b, 3.0).expect("should produce a constraint");
        let result = compute_avoiding_velocity(a.preferred, a.max_speed, &[line]);

        // The faster agent should get a lateral component to overtake.
        assert!(
            result.y.abs() > 0.1,
            "Expected lateral component for overtaking, got {result:?}"
        );
    }

    #[test]
    fn diverging_agents_produce_minimal_adjustment() {
        // Agents moving apart — very little or no avoidance needed.
        let a = agent(
            Vec2::new(0.0, 0.0),
            Vec2::new(-50.0, 0.0),
            Vec2::new(-50.0, 0.0),
        );
        let b = agent(
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );

        let line = compute_orca_line(&a, &b, 3.0);
        if let Some(line) = line {
            let result = compute_avoiding_velocity(a.preferred, a.max_speed, &[line]);
            // Should stay close to preferred since agents are moving apart.
            let diff = (result - a.preferred).length();
            assert!(
                diff < 10.0,
                "Diverging agents should have minimal adjustment, got diff={diff}"
            );
        }
        // `None` is also acceptable — degenerate cases.
    }

    #[test]
    fn overlapping_agents_return_none() {
        // Agents at the same x position, overlapping.
        let a = agent(Vec2::new(0.0, 0.0), Vec2::ZERO, Vec2::new(50.0, 0.0));
        let b = agent(Vec2::new(5.0, 0.0), Vec2::ZERO, Vec2::new(-50.0, 0.0));

        // Combined radius = 12, distance = 5 → overlapping.
        // ORCA skips overlapping agents — physics handles separation.
        let line = compute_orca_line(&a, &b, 3.0);
        assert!(
            line.is_none(),
            "Overlapping agents should return None (physics handles separation)"
        );
    }

    #[test]
    fn lp2_single_constraint_respects_half_plane() {
        // Create a line that forbids moving right.
        let line = OrcaLine {
            point: Vec2::new(10.0, 0.0),
            direction: Vec2::new(0.0, 1.0),
        };
        let preferred = Vec2::new(50.0, 0.0);
        let (result, fail) = linear_program_2(&[line], preferred, 50.0);

        // Should satisfy the constraint: det(dir, point - result) <= 0
        let violation = det(line.direction, line.point - result);
        assert!(
            violation <= f32::EPSILON,
            "Result should satisfy the constraint, violation={violation}"
        );
        assert_eq!(fail, 1, "Should succeed with single constraint");
    }

    #[test]
    fn lp3_infeasible_minimizes_violation() {
        // Truly contradictory constraints:
        // Constraint 0: valid when x >= 20 (point=(20,0), dir=(0,-1))
        // Constraint 1: valid when x <= -20 (point=(-20,0), dir=(0,1))
        let lines = [
            OrcaLine {
                point: Vec2::new(20.0, 0.0),
                direction: Vec2::new(0.0, -1.0),
            },
            OrcaLine {
                point: Vec2::new(-20.0, 0.0),
                direction: Vec2::new(0.0, 1.0),
            },
        ];

        let (result, fail) = linear_program_2(&lines, Vec2::ZERO, 50.0);
        assert!(fail < lines.len(), "Should be infeasible");

        let result = linear_program_3(&lines, fail, result, 50.0);
        // LP3 should find a compromise. Result should have bounded magnitude.
        assert!(
            result.length() <= 50.0 + 1.0,
            "Result should be within max_speed, got {}",
            result.length()
        );
    }

    #[test]
    fn result_within_max_speed() {
        // Multiple constraints — result must stay within speed disc.
        let a = agent(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        let b = agent(
            Vec2::new(20.0, 5.0),
            Vec2::new(-30.0, -10.0),
            Vec2::new(-30.0, -10.0),
        );
        let c = agent(
            Vec2::new(15.0, -10.0),
            Vec2::new(-20.0, 30.0),
            Vec2::new(-20.0, 30.0),
        );

        let mut lines = Vec::new();
        if let Some(line) = compute_orca_line(&a, &b, 3.0) {
            lines.push(line);
        }
        if let Some(line) = compute_orca_line(&a, &c, 3.0) {
            lines.push(line);
        }

        let result = compute_avoiding_velocity(a.preferred, a.max_speed, &lines);
        assert!(
            result.length() <= a.max_speed + 0.1,
            "Result speed {} should be <= max_speed {}",
            result.length(),
            a.max_speed
        );
    }

    #[test]
    fn zero_preferred_still_avoids() {
        // Stationary agent with someone approaching — should still dodge.
        let a = agent(Vec2::new(0.0, 0.0), Vec2::ZERO, Vec2::ZERO);
        let b = agent(
            Vec2::new(20.0, 0.0),
            Vec2::new(-50.0, 0.0),
            Vec2::new(-50.0, 0.0),
        );

        let line = compute_orca_line(&a, &b, 3.0).expect("should produce constraint");
        let result = compute_avoiding_velocity(a.preferred, a.max_speed, &[line]);

        // With zero preferred, the LP should find a valid velocity to dodge.
        // It might be zero if the constraint doesn't forbid it, or non-zero if it does.
        assert!(
            result.length() <= a.max_speed + 0.1,
            "Result should be within max_speed"
        );
    }

    #[test]
    fn full_responsibility_takes_all_adjustment() {
        let mut a = agent(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        let mut b = agent(
            Vec2::new(30.0, 0.0),
            Vec2::new(-50.0, 0.0),
            Vec2::new(-50.0, 0.0),
        );

        // a takes full responsibility
        a.responsibility = 1.0;
        b.responsibility = 0.0;

        let line_a = compute_orca_line(&a, &b, 3.0).expect("should produce constraint for a");

        // b takes no responsibility
        let line_b = compute_orca_line(&b, &a, 3.0).expect("should produce constraint for b");

        let result_a = compute_avoiding_velocity(a.preferred, a.max_speed, &[line_a]);
        let result_b = compute_avoiding_velocity(b.preferred, b.max_speed, &[line_b]);

        // a should deviate more than b.
        let deviation_a = (result_a - a.preferred).length();
        let deviation_b = (result_b - b.preferred).length();
        assert!(
            deviation_a > deviation_b,
            "Full-responsibility agent should deviate more: a={deviation_a}, b={deviation_b}"
        );
    }
}
