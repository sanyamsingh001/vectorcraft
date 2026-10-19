"""Private remote control proposal. Never interprets an expected failure as test PASS."""
from pathlib import Path
import hashlib, json, os, platform, re, subprocess, sys, time

SOURCES = {
    'base': ('01e165af633b88cb9df55d6a24ac4567e9fe9c06', '5c5fe0910928c520fb33273df766b3de5cb6be0a'),
    'published': ('b4f183eb7e3dfcd4d97c5995a9c227430c2235ec', '22246235e197656f61dce82193e76852eaebef22'),
}
TEST = 'tests_vertical::mojikumi_halves_line_end_and_consecutive_punctuation'
TEST_FILE = 'crates/text/src/tests_vertical.rs'
ORIGINAL_SHA = 'df289f09c38ab469aa8559598dc5ca3496a386b85c9079ef9fc0e0325fa8331f'
DIAGNOSTIC_SHA = 'b3de1e1a919a41fd3a3963d0be0a9de9ffca823dbfdfa769bc878c79b016e119'
PATCH_SHA = 'e5174cb17caf472155b56b4417f5be914eb000d83a2b2e5ee75a739e26efb338'
ARABIC = ['arabic_contextual_forms_and_lam_alef_use_the_shaper', 'identical_style_arabic_run_splits_keep_joining_and_source_attribution', 'different_style_arabic_boundary_receives_joining_context']
BIDI_NAMES = ['tests_bidi::arabic_contextual_forms_and_lam_alef_use_the_shaper', 'tests_bidi::automatic_alignment_follows_each_paragraph_and_explicit_choices_win', 'tests_bidi::bidi_isolates_numbers_and_brackets_keep_valid_clusters', 'tests_bidi::craft_arabic_fonts_load_and_noto_sans_arabic_is_the_arabic_fallback', 'tests_bidi::different_style_arabic_boundary_receives_joining_context', 'tests_bidi::hebrew_and_arabic_are_visually_rtl_with_exact_source_clusters', 'tests_bidi::hebrew_marks_stay_with_base_clusters', 'tests_bidi::identical_style_arabic_run_splits_keep_joining_and_source_attribution', 'tests_bidi::left_to_right_text_is_untouched', 'tests_bidi::mixed_text_preserves_latin_and_number_order', 'tests_bidi::paragraph_direction_is_independent_and_neutrals_do_not_force_ltr', 'tests_bidi::paragraph_direction_is_per_paragraph', 'tests_bidi::paragraph_direction_overrides_the_first_strong_character', 'tests_bidi::right_aligned_wrapped_rtl_keeps_trailing_spaces_outside_content', 'tests_bidi::rtl_hit_testing_arrows_and_selection_follow_visual_positions', 'tests_bidi::shape_range_handles_empty_runs_and_selected_utf8_subranges', 'tests_bidi::visual_arrows_cross_rtl_paragraph_boundaries', 'tests_bidi::visual_text_comes_back_in_logical_order', 'tests_bidi::wrapping_retains_logical_line_ranges_and_reorders_each_line']
ENV_KEYS = ['RUSTUP_TOOLCHAIN', 'CARGO_BUILD_JOBS', 'CARGO_PROFILE_DEV_DEBUG', 'CARGO_PROFILE_TEST_DEBUG', 'CARGO_INCREMENTAL', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUST_MIN_STACK', 'RUST_TEST_THREADS', 'PROPTEST_CASES', 'CRAFT_FONTS_DIR', 'CRAFT_FONTS_REQUIRED']
assert len(sys.argv) == 4, 'base checkout, published checkout, fresh evidence directory required'
roots = dict(zip(SOURCES, [Path(p).resolve() for p in sys.argv[1:3]]))
out = Path(sys.argv[3]).resolve()
out.mkdir(parents=True, exist_ok=False)
patch = Path(__file__).with_name('mojikumi-metrics.patch')
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(patch) == PATCH_SHA
(out / patch.name).write_bytes(patch.read_bytes())

def save(name, value):
    (out / name).write_text(json.dumps(value, indent=2) + '\n')

def git(role, *args):
    return subprocess.check_output(['git', *args], cwd=roots[role])

def guard(role, diagnostic=False):
    head, tree = SOURCES[role]
    assert git(role, 'rev-parse', 'HEAD').decode().strip() == head
    assert git(role, 'rev-parse', 'HEAD^{tree}').decode().strip() == tree
    assert not git(role, 'diff', '--cached', '--name-only')
    names = git(role, 'diff', '--name-only', 'HEAD').decode().splitlines()
    assert names == ([TEST_FILE] if diagnostic else [])
    assert sha(roots[role] / TEST_FILE) == (DIAGNOSTIC_SHA if diagnostic else ORIGINAL_SHA)
    diff = git(role, 'diff', '--binary', '--full-index', 'HEAD')
    return {'head': head, 'tree': tree, 'source_kind': 'temporary_diagnostic' if diagnostic else 'unchanged',
            'test_file_sha256': sha(roots[role] / TEST_FILE), 'diff_sha256': hashlib.sha256(diff).hexdigest(),
            'cargo_lock_sha256': sha(roots[role] / 'Cargo.lock')}

for key in ['RUST_MIN_STACK', 'RUST_TEST_THREADS', 'PROPTEST_CASES', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CRAFT_FONTS_DIR', 'CRAFT_FONTS_REQUIRED']:
    assert not os.environ.get(key), key
for key, value in {'RUSTUP_TOOLCHAIN': '1.99.0', 'CARGO_BUILD_JOBS': '1', 'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0', 'CARGO_INCREMENTAL': '0'}.items():
    assert os.environ.get(key) == value, key
rust = subprocess.check_output(['rustc', '-vV'], text=True)
assert rust.startswith('rustc 1.99.0 ')
save('environment.json', {'rust': rust, 'cargo': subprocess.check_output(['cargo', '-vV'], text=True),
     'platform': platform.platform(), 'environment': {k: os.environ.get(k) for k in ENV_KEYS},
     'runner_sha256': sha(Path(__file__)), 'initial_sources': {r: guard(r) for r in SOURCES}})
# Preserve installed-package evidence BEFORE any possibly failing test. No font bytes are uploaded.
packages = ['fonts-noto-core', 'fonts-dejavu-core']
versions = subprocess.check_output(['dpkg-query', '-W', '-f=${Package} ${Version}\n', *packages], text=True)
package_paths = subprocess.check_output(['dpkg-query', '-L', *packages], text=True).splitlines()
font_paths = sorted({Path(p) for p in package_paths if Path(p).suffix.lower() in ('.ttf', '.otf', '.ttc') and Path(p).is_file()})
fonts = [{'path': str(p), 'sha256': sha(p)} for p in font_paths]
assert any(p.name == 'NotoSansArabic-Regular.ttf' for p in font_paths)
font_evidence = {'packages': versions, 'font_files': fonts}
save('font-environment.json', font_evidence)
results = []

def run(role, label, command, diagnostic=False):
    pre = guard(role, diagnostic)
    log, err = out / (label + '.log'), out / (label + '.stderr.log')
    started = time.time()
    print('COMMAND', role, label, json.dumps(command), flush=True)
    # Kuhn's raw-stream approach: direct binary files, never stderr=STDOUT.
    with log.open('xb') as stdout, err.open('xb') as stderr:
        status = subprocess.run(command, cwd=roots[role], stdout=stdout, stderr=stderr).returncode
    post = guard(role, diagnostic)
    assert post == pre
    row = {'role': role, 'label': label, 'command': command, 'exit_code': status,
           'started': started, 'ended': time.time(), 'source_before': pre, 'source_after': post,
           'source_unchanged': not diagnostic, 'diagnostic_bytes_unchanged': diagnostic,
           'log': log.name, 'log_stream': 'stdout', 'log_sha256': sha(log),
           'stderr_log': err.name, 'stderr_log_sha256': sha(err)}
    results.append(row)
    save('results.json', results)
    return row

def streams(row):
    for name, digest in [(row['log'], row['log_sha256']), (row['stderr_log'], row['stderr_log_sha256'])]:
        assert sha(out / name) == digest
    return (out / row['log']).read_text(), (out / row['stderr_log']).read_text()

exact = ['cargo', 'test', '-p', 'vectorcraft-text', '--lib', TEST, '--', '--exact', '--nocapture']
originals = [run(role, role + '-original-mojikumi', exact) for role in SOURCES]
bidi = run('published', 'published-bidi', ['cargo', 'test', '-p', 'vectorcraft-text', '--lib', 'tests_bidi::', '--', '--nocapture'])
pdf = run('published', 'published-pdf', ['cargo', 'test', '-p', 'vectorcraft-pdf', '--test', 'system_fonts', '--test', 'embedded_system_fonts', '--', '--nocapture'])
bout, berr = streams(bidi)
pout, perr = streams(pdf)
names = re.findall(r'^test (tests_bidi::\w+) \.\.\. ok$', bout, re.M)
positive = {n: ('ASSERTIONS RUN: ' + n + '; Arabic font Noto Sans Arabic ') in berr and ('SKIPPED: ' + n + ';') not in berr for n in ARABIC}
bidi_ok = bidi['exit_code'] == 0 and len(names) == len(set(names)) == 19 and set(names) == set(BIDI_NAMES) and 'test result: ok. 19 passed; 0 failed; 0 ignored;' in bout and all(positive.values())
pdf_ok = (pdf['exit_code'] == 0 and all('Running tests/' + n + '.rs (' in perr for n in ['system_fonts', 'embedded_system_fonts'])
          and pout.count('test a_fresh_session_resolves_pdf_fonts_to_installed_families ... ok') == 2
          and pout.count('test result: ok. 1 passed; 0 failed; 0 ignored;') == 2
          and all('PDF system-font import: case=' + c + ',' in perr for c in ['packed-family', 'PostScript'])
          and 'nothing to check' not in pout + perr)
save('focused-guard.json', {'bidi19_accepted': bidi_ok, 'arabic_positive_prefixes': positive,
     'optional_craft_skip': 'skipping: built without craft-fonts' in berr, 'pdf_two_binaries_accepted': pdf_ok,
     'font_environment_sha256': sha(out / 'font-environment.json'), 'bidi_result': bidi, 'pdf_result': pdf})
# Failures above are retained, but do not suppress evidence from the remaining independent sections.
original_checks = []
for row in originals:
    stdout, stderr = streams(row)
    seen = bool(re.search(r'^test ' + re.escape(TEST) + r' \.\.\. (ok|FAILED)$', stdout, re.M))
    same_failure = (row['exit_code'] != 0 and 'test result: FAILED. 0 passed; 1 failed; 0 ignored;' in stdout
                    and 'crates/text/src/tests_vertical.rs:351:9:' in stderr and '\nvertical false\n' in stderr)
    original_checks.append({'role': row['role'], 'exact_test_seen': seen, 'same_line351_horizontal_failure': same_failure})
# These diagnostics cannot replace either original run. No extra layout calls or changed assertions.
if all(c['exact_test_seen'] for c in original_checks):
    for role in SOURCES:
        test_file = roots[role] / TEST_FILE
        original_bytes = test_file.read_bytes()
        guard(role)
        try:
            subprocess.run(['git', 'apply', '--check', str(patch)], cwd=roots[role], check=True)
            subprocess.run(['git', 'apply', str(patch)], cwd=roots[role], check=True)
            guard(role, True)
            (out / (role + '-diagnostic-source.patch')).write_bytes(git(role, 'diff', '--binary', '--full-index', 'HEAD'))
            run(role, role + '-diagnostic-mojikumi', exact, True)
        finally:
            test_file.write_bytes(original_bytes)
            guard(role)
else:
    save('diagnostics-unavailable.json', {'reason': 'Both exact original tests must execute before diagnostic source changes are allowed.', 'original_checks': original_checks})
assert fonts == [{'path': str(p), 'sha256': sha(p)} for p in font_paths]
assert versions == subprocess.check_output(['dpkg-query', '-W', '-f=${Package} ${Version}\n', *packages], text=True)
summary = {'original_controls': original_checks, 'same_baseline_failure_demonstrated': all(c['same_line351_horizontal_failure'] for c in original_checks),
           'focused_positive_guard': bidi_ok and pdf_ok, 'full_ci_run': False,
           'final_clean_sources': {r: guard(r) for r in SOURCES}, 'font_environment_unchanged': True,
           'command_exit_codes': {r['label']: r['exit_code'] for r in results}}
save('summary.json', summary)
print(json.dumps(summary, indent=2), flush=True)
# An expected baseline failure is still a nonzero command and a nonzero workflow outcome.
sys.exit(0 if all(r['exit_code'] == 0 for r in results) and bidi_ok and pdf_ok and all(c['exact_test_seen'] for c in original_checks) else 1)
