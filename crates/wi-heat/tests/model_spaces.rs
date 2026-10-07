//! `spaces.test.ts`, case for case. (The sidebar's group heading and the empty
//! Tasks line are display and stay in TypeScript; so do their assertions.)

mod common;

use common::*;
use wi_heat::model::records::{
    Capture, Course, GroupKind, Milestone, Project, ProjectStatus, Space, Task,
};
use wi_heat::model::spaces::{
    default_spaces, group_counts, group_name, in_space, library_lists, space_counts, GroupCount,
    SidebarData, SpaceCounts, SpaceOpen,
};

fn data(over: impl FnOnce(&mut SidebarData)) -> SidebarData {
    let mut d = SidebarData::default();
    over(&mut d);
    d
}

fn titles(tasks: &[Task]) -> Vec<&str> {
    tasks.iter().map(|t| t.title.as_str()).collect()
}

// --- the three default spaces (3.1, 3.4) ----------------------------------------------------

#[test]
fn are_classes_wwav_and_personal_with_todays_groups_and_types_plus_other() {
    let spaces = default_spaces(&mut ids("space"));
    let got: Vec<(&str, GroupKind, &str, Vec<&str>)> = spaces
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.group_kind,
                s.group_label.as_str(),
                s.types.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (
                "Classes",
                GroupKind::Course,
                "Course",
                vec![
                    "Homework",
                    "Quiz",
                    "Listening",
                    "Reading",
                    "Lab",
                    "Project",
                    "Exam prep",
                    "Other"
                ]
            ),
            (
                "WWAV",
                GroupKind::Milestone,
                "Milestone",
                vec!["Hardware", "Software", "Design", "Music", "Business", "Content", "Other"]
            ),
            (
                "Personal",
                GroupKind::Free,
                "Area",
                vec!["Errand", "Admin", "Money", "Health", "Home", "Social", "Other"]
            ),
        ]
    );
    assert_eq!(
        spaces.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["space-1", "space-2", "space-3"]
    );
}

#[test]
fn give_wwav_the_persona_rewritten_to_cover_the_app() {
    let spaces = default_spaces(&mut ids("space"));
    let (classes, wwav, personal) = (&spaces[0], &spaces[1], &spaces[2]);
    assert_eq!(
        wwav.persona,
        "a solo founder building WWAV: the PRANA handheld (Teensy 4.1, C++ firmware, PCBs), the Wi_WWAV desktop app, and an album."
    );
    assert_ne!(classes.persona, "");
    assert_ne!(personal.persona, "");
}

#[test]
fn have_three_different_hues() {
    let spaces = default_spaces(&mut ids("space"));
    let mut hues: Vec<f64> = spaces.iter().map(|s| s.hue).collect();
    hues.sort_by(f64::total_cmp);
    hues.dedup();
    assert_eq!(hues.len(), 3);
}

// --- filters --------------------------------------------------------------------------------------

#[test]
fn keep_a_spaces_tasks_or_every_task_under_all() {
    let a = task(|t| t.space_id = "classes".into());
    let b = task(|t| t.space_id = "wwav".into());
    let all = [a.clone(), b.clone()];
    let in_wwav = in_space(Some("wwav"));
    assert_eq!(
        all.iter()
            .filter(|t| in_wwav(t))
            .cloned()
            .collect::<Vec<_>>(),
        vec![b.clone()]
    );
    let everything = in_space(None);
    assert_eq!(
        all.iter()
            .filter(|t| everything(t))
            .cloned()
            .collect::<Vec<_>>(),
        vec![a, b]
    );
}

#[test]
fn count_each_spaces_open_tasks_and_all_of_them() {
    let now = ny("2026-10-06 08:40");
    let spaces: Vec<Space> = default_spaces(&mut ids("s"));
    let tasks = vec![
        task(|t| t.space_id = "s-1".into()),
        task(|t| t.space_id = "s-1".into()),
        task(|t| {
            t.space_id = "s-1".into();
            t.done = true;
            t.done_at = Some(now);
        }),
        task(|t| t.space_id = "s-2".into()),
        // A series that has ended isn't open.
        task(|t| {
            t.space_id = "s-3".into();
            t.rrule = Some("FREQ=DAILY;COUNT=1".into());
            t.due = Some(ny("2026-10-01 09:00"));
        }),
    ];
    assert_eq!(
        space_counts(&spaces, &tasks, &[], now, &ny_zone()),
        SpaceCounts {
            all: 3,
            spaces: vec![
                SpaceOpen {
                    space: spaces[0].clone(),
                    open: 2
                },
                SpaceOpen {
                    space: spaces[1].clone(),
                    open: 1
                },
                SpaceOpen {
                    space: spaces[2].clone(),
                    open: 0
                },
            ],
        }
    );
}

// --- the Tasks sidebar (3.6) --------------------------------------------------------------------------

#[test]
fn lists_inbox_all_open_hot_due_this_week_scheduled_someday_and_done() {
    let now = ny("2026-10-06 08:40");
    let someday = Project {
        id: "p1".into(),
        space_id: "wwav".into(),
        title: "Vinyl run".into(),
        status: ProjectStatus::Someday,
        target_date: None,
        link: None,
        ..Default::default()
    };
    let active = Project {
        id: "p2".into(),
        space_id: "wwav".into(),
        title: "Album".into(),
        status: ProjectStatus::Active,
        target_date: None,
        link: None,
        ..Default::default()
    };
    let titled = |title: &str, over: &dyn Fn(&mut Task)| {
        task(|t| {
            t.title = title.into();
            over(t);
        })
    };
    let hot = titled("hot", &|t| {
        t.difficulty = 3.0;
        t.due = Some(now + DAY_MS);
    });
    let week = titled("week", &|t| {
        t.difficulty = 1.0;
        t.due = Some(now + 6.0 * DAY_MS);
    });
    let late = titled("late", &|t| t.due = Some(now - DAY_MS));
    let scheduled = titled("scheduled", &|t| {
        t.scheduled_date = Some("2026-10-08".into())
    });
    let parked = titled("parked", &|t| t.project_id = Some(someday.id.clone()));
    let live = titled("live", &|t| t.project_id = Some(active.id.clone()));
    let done = titled("done", &|t| {
        t.done = true;
        t.done_at = Some(now);
    });
    let captures = vec![
        Capture {
            id: "c1".into(),
            text: "fix the snare at 1:32".into(),
            link: None,
            triaged_at: None,
            result_type: None,
            result_id: None,
        },
        Capture {
            id: "c2".into(),
            text: "old".into(),
            link: None,
            triaged_at: Some(now),
            result_type: None,
            result_id: None,
        },
    ];
    let d = data(|d| {
        d.tasks = vec![hot, week, late, scheduled, parked, live, done];
        d.captures = captures.clone();
        d.projects = vec![someday, active];
    });
    let lists = library_lists(&d, now, &ny_zone(), None);
    assert_eq!(lists.inbox, vec![captures[0].clone()]);
    assert_eq!(
        titles(&lists.all_open),
        ["late", "hot", "week", "scheduled", "parked", "live"]
    );
    assert_eq!(titles(&lists.hot), ["late", "hot"]);
    assert_eq!(titles(&lists.due_this_week), ["late", "hot", "week"]);
    assert_eq!(titles(&lists.scheduled), ["scheduled"]);
    assert_eq!(titles(&lists.someday), ["parked"]);
    assert_eq!(titles(&lists.done), ["done"]);
}

#[test]
fn applies_the_space_filter_to_tasks_but_not_to_the_inbox() {
    let now = ny("2026-10-06 08:40");
    let captures = vec![Capture {
        id: "c1".into(),
        text: "a thought".into(),
        link: None,
        triaged_at: None,
        result_type: None,
        result_id: None,
    }];
    let d = data(|d| {
        d.tasks = vec![task(|t| t.space_id = "classes".into())];
        d.captures = captures.clone();
    });
    let lists = library_lists(&d, now, &ny_zone(), Some("wwav"));
    assert_eq!(lists.all_open, vec![]);
    assert_eq!(lists.inbox, captures);
}

// --- groups -------------------------------------------------------------------------------------------------

#[test]
fn lists_courses_milestones_or_areas_under_each_space_with_open_counts() {
    let now = ny("2026-10-06 08:40");
    let spaces = default_spaces(&mut ids("s"));
    let (classes, wwav, personal) = (&spaces[0], &spaces[1], &spaces[2]);
    let course = |id: &str, code: &str, name: &str| Course {
        id: id.into(),
        term_id: "t".into(),
        code: code.into(),
        name: name.into(),
        categories: vec![],
        scale: None,
        notes: String::new(),
        ..Default::default()
    };
    let (jpn, mth) = (
        course("jpn201", "JPN 201", "Japanese"),
        course("mth142", "MTH 142", "Calculus"),
    );
    let bead = |id: &str, title: &str, date: &str, order: f64| Milestone {
        id: id.into(),
        space_id: wwav.id.clone(),
        project_id: None,
        title: title.into(),
        date: date.into(),
        done: false,
        order,
        link: None,
        ..Default::default()
    };
    let beads = vec![
        bead("m2", "Firmware 1.0", "2026-11-01", 2.0),
        bead("m1", "Enclosure v2", "2026-10-20", 1.0),
    ];
    let finished = |over: &dyn Fn(&mut Task)| {
        task(|t| {
            over(t);
            t.done = true;
            t.done_at = Some(now);
        })
    };
    let tasks = vec![
        task(|t| {
            t.space_id = classes.id.clone();
            t.course_id = Some(mth.id.clone());
        }),
        task(|t| {
            t.space_id = classes.id.clone();
            t.course_id = Some(jpn.id.clone());
        }),
        task(|t| {
            t.space_id = classes.id.clone();
            t.course_id = Some(jpn.id.clone());
        }),
        finished(&|t| {
            t.space_id = classes.id.clone();
            t.course_id = Some(jpn.id.clone());
        }),
        task(|t| {
            t.space_id = wwav.id.clone();
            t.milestone_id = Some("m2".into());
        }),
        task(|t| {
            t.space_id = personal.id.clone();
            t.group = Some("Car".into());
        }),
        task(|t| {
            t.space_id = personal.id.clone();
            t.group = Some("Apartment".into());
        }),
        finished(&|t| {
            t.space_id = personal.id.clone();
            t.group = Some("Car".into());
        }),
    ];
    let d = data(|d| {
        d.tasks = tasks.clone();
        d.milestones = beads.clone();
        d.courses = vec![mth.clone(), jpn.clone()];
    });
    let z = ny_zone();
    let count = |id: &str, name: &str, open: usize| GroupCount {
        id: id.into(),
        name: name.into(),
        open,
    };
    assert_eq!(
        group_counts(classes, &d, now, &z),
        vec![count("jpn201", "JPN 201", 2), count("mth142", "MTH 142", 1)]
    );
    assert_eq!(
        group_counts(wwav, &d, now, &z),
        vec![
            count("m1", "Enclosure v2", 0),
            count("m2", "Firmware 1.0", 1)
        ]
    );
    assert_eq!(
        group_counts(personal, &d, now, &z),
        vec![count("Apartment", "Apartment", 1), count("Car", "Car", 1)]
    );
}

#[test]
fn names_a_tasks_group_as_the_lcd_shows_it_the_course_code_the_milestone_or_the_area() {
    let spaces = default_spaces(&mut ids("s"));
    let jpn = Course {
        id: "jpn201".into(),
        term_id: "t".into(),
        code: "JPN 201".into(),
        name: "Japanese".into(),
        categories: vec![],
        scale: None,
        notes: String::new(),
        ..Default::default()
    };
    let bead = Milestone {
        id: "m2".into(),
        space_id: spaces[1].id.clone(),
        project_id: None,
        title: "Firmware 1.0".into(),
        date: "2026-11-01".into(),
        done: false,
        order: 2.0,
        link: None,
        ..Default::default()
    };
    let courses = [jpn.clone()];
    let beads = [bead];
    assert_eq!(
        group_name(
            &task(|t| t.course_id = Some(jpn.id.clone())),
            &courses,
            &beads
        ),
        Some("JPN 201".into())
    );
    assert_eq!(
        group_name(
            &task(|t| t.milestone_id = Some("m2".into())),
            &courses,
            &beads
        ),
        Some("Firmware 1.0".into())
    );
    assert_eq!(
        group_name(&task(|t| t.group = Some("Car".into())), &courses, &beads),
        Some("Car".into())
    );
    assert_eq!(group_name(&task(|_| {}), &courses, &beads), None);
}
