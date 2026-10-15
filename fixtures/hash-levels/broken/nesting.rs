//! Parses cleanly, but its use runs nest 73 deep through attribute values, past
//! `MAX_USE_RUN_NESTING` (64): the function and the file module are `nesting_too_deep`.
fn f() {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
#[a = {
use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
0 }] use x; use y;
}
