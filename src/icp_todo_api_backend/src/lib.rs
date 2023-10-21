use candid::CandidType;
use ic_cdk::api::caller as caller_api;
use ic_cdk::export::{candid, Principal};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::cell::RefCell;
use ic_cdk_macros::*;

type PrincipalName = String;

#[derive(Clone, CandidType, Serialize, Deserialize)]
pub struct Todo {
    id: u128,
    task: String,
}

#[derive(Clone, CandidType, Serialize, Deserialize)]
struct CanisterState {
    counter: u128,
    todos: BTreeMap<PrincipalName, Vec<Todo>>,
}
//2286474 IDID

thread_local! {
    // Currently, a single canister smart contract is limited to 4 GB of storage due to WebAssembly limitations.
    // To ensure that our canister does not exceed this limit, we restrict memory usage to at most 2 GB because 
    // up to 2x memory may be needed for data serialization during canister upgrades. Therefore, we aim to support
    // up to 1,000 users, each storing up to 2 MB of data.
    // The data is reserved for storing the todos:
    //     TODOS_PER_USER = MAX_TODOS_PER_USER x MAX_TODO_CHARS x (4 bytes per char)
    //     2 MB = 500 x 1000 x 4 = 2,000,000

    // Define dapp limits - important for security assurance
    static MAX_USERS: usize = 1_000;
    static MAX_TODO_PER_USER: usize = 500;
    static MAX_TODO_CHARS: usize = 1000;

    pub static NEXT_TODO: RefCell<u128> = RefCell::new(0);
    pub static TODO_BY_USER: RefCell<BTreeMap<PrincipalName, Vec<Todo>>> = RefCell::new(BTreeMap::new());
}

fn caller() -> Principal {
    caller_api()
}

#[init]
fn init() {}

#[update(name = "whoami")]
fn whoami() -> String {
    caller_api().to_string()
}

/// Returns the current number of users.
fn user_count() -> usize {
    TODO_BY_USER.with(|todo_ref| todo_ref.borrow().keys().len())
}

fn is_id_sane(id: u128) -> bool {
    MAX_TODO_PER_USER.with(|max_todo_per_user| id < (*max_todo_per_user as u128) * (user_count() as u128))
}

/// Returns (a future of) this [caller]'s todos.
/// Panics: 
///     [caller] is the anonymous identity
///     [caller] is not a registered user
#[query(name = "get_todos")]
fn get_todos() -> Vec<Todo> {
    let user = caller();
    let user_str = user.to_string();
    TODO_BY_USER.with(|todo_ref| {
        todo_ref
            .borrow()
            .get(&user_str)
            .cloned()
            .unwrap_or_default()
    })
}

/// Delete this [caller]'s todo with given id. If none of the 
/// existing todos have this id, do nothing. 
/// [id]: the id of the todo to be deleted
///
/// Returns: 
///      Future of unit
/// Panics: 
///      [caller] is the anonymous identity
///      [caller] is not a registered user
///      [id] is unreasonable; see [is_id_sane]
#[update(name = "delete_todo")]
fn delete_todo(todo_id: u128) {
    let user = caller();
    assert!(is_id_sane(todo_id));

    let user_str = user.to_string();
    // shared ownership borrowing
    TODO_BY_USER.with(|todo_ref| {
        let mut writer = todo_ref.borrow_mut();
        if let Some(v) = writer.get_mut(&user_str) {
            v.retain(|item| item.id != todo_id);
        }
    });
}

/// Returns (a future of) this [caller]'s todos.
/// Panics: 
///     [caller] is the anonymous identity
///     [caller] is not a registered user
///     [todo.task] exceeds [MAX_TODO_CHARS]
///     [todo.id] is unreasonable; see [is_id_sane]
#[update(name = "update_todo")]
fn update_todo(todos: Todo) {
    let user = caller();
    assert!(todos.task.chars().count() <= MAX_TODO_CHARS.with(|mnc| *mnc));
    assert!(is_id_sane(todos.id));

    let user_str = user.to_string();
    TODO_BY_USER.with(|todos_ref| {
        let mut writer = todos_ref.borrow_mut();
        if let Some(old_todo) = writer
            .get_mut(&user_str)
            .and_then(|td| td.iter_mut().find(|t| t.id == todos.id))
        {
            old_todo.task = todos.task;
        }
    })
}

/// Add new todo for this [caller].
///      [todo]: (encrypted) content of this todo
///
/// Returns: 
///      Future of unit
/// Panics: 
///      [caller] is the anonymous identity
///      [caller] is not a registered user
///      [todo] exceeds [MAX_TODO_CHARS]
///      User already has [MAX_TODOS_PER_USER] todos
///      [todo] would be for a new user and [MAX_USERS] is exceeded
#[update(name = "add_todo")]
fn add_todo(task: String) {
    let user = caller();
    assert!(task.chars().count() <= MAX_TODO_CHARS.with(|mtc| *mtc));

    let user_str = user.to_string();
    let todo_id = NEXT_TODO.with(|counter_ref| {
        let mut writer = counter_ref.borrow_mut();
        *writer += 1;
        *writer
    });

    let user_count = user_count();
    TODO_BY_USER.with(|todos_ref| {
        let mut writer = todos_ref.borrow_mut();
        let user_todos = writer.entry(user_str).or_insert_with(|| {
            // caller unknown ==> check invariants
            // A. can we add a new user?
            assert!(MAX_USERS.with(|mu| user_count < *mu));
            vec![]
        });

        assert!(user_todos.len() < MAX_TODO_PER_USER.with(|mtpu| *mtpu));

        user_todos.push(Todo {
            id: todo_id,
            task: task,
        });
    });
}