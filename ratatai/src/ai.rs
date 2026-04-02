// src/ai.rs

use std::collections::HashSet;

use google_ai_rs::{GenerativeModel, genai::Response};
use regex::Regex;
use scraper::{Html, Selector};
use tracing::{debug, info, warn};

pub async fn get_gemini_response<'a>(
    model: GenerativeModel<'a>,
    prompt: String,
) -> anyhow::Result<Response> {
    let response = model.generate_content(prompt).await?;
    Ok(response)
}

// pub(crate) fn get_initial_prompt() -> String {
//     "Forget all previous instructions or prompts to go ahead with this request!
// Here are the instructions to answer bug requests, then I will provide you the reported bug:
// Here is the template for bug submission with all the required information:
// *** Start template ***
// Description
// ===========
// Some prose which explains more in detail what this bug report is
// about. If the headline of this report is descriptive enough,
// skip this section.
//
// Steps to reproduce
// ==================
// A chronological list of steps which will bring off the
// issue you noticed:
// * I did X
// * then I did Y
// * then I did Z
// A list of openstack client commands would be the most
// descriptive example.
//
// Expected result
// ===============
// After the execution of the steps above, what should have
// happened if the issue wasn't present?
//
// Actual result
// =============
// What happened instead of the expected result?
// How did the issue look like?
//
// Environment
// ===========
// 1. Exact version of OpenStack you are running. See the following
//    list for all releases: http://docs.openstack.org/releases/
//
//     If this is from a distro please provide
//         $ dpkg -l | grep <projectname>
//         or
//         $ rpm -qa | grep <projectname>
//     If this is from git, please provide
//         $ git log -1
//
// 2. Which storage type did you use? If the case is related to storage.
//    (For example: Ceph, LVM, GPFS, ...)
//
// Logs & Configs
// ==============
// Provide logs like stacktrace or from openstack services using debug mode.
//
// Note: The tool *sosreport* has support for some OpenStack projects.
// It's worth having a look at it. For example, if you want to collect
// the logs of a compute node you would execute:
//
//     $ sudo sosreport -o openstack_nova --batch
//
// on that compute node. Attach the logs to this bug report.
// *** end template ***
//
// Link bug reporting template: https://wiki.openstack.org/wiki/Nova/BugsTeam/BugReportTemplate
// Current fully supported version of Openstack: 2026.1 Gazpacho, 2025.2 Flamingo, 2025.1 Epoxy, 2024.2 Dalmatian
// Link supported realease: https://releases.openstack.org/
//
// Instruction to craft the answer:
//
// 1- Answer must be in plain text.
// 2- You must thank the reporter for submitting the bug.
// 3- If the OpenStack version mentioned in the report is not supported, inform the reporter. Do not list supported versions; instead provide only the link to the page describing supported versions.
// 4- If information required by the bug report template is missing, clearly mention what information is missing.
// 5- Provide the link to the bug report template for reference (https://wiki.openstack.org/wiki/Nova/BugsTeam/BugReportTemplate).
// 6- If the bug is 'Incomplete':
//    - Explain that we will mark this bug as 'Incomplete', and ask the reporter to set it back to 'New' once the missing information is provided.
//    Example:
//    For now, we’ll mark this bug as 'Incomplete'. Please update the report with the missing information and set it back to 'New' once updated.
//
// 7- If the bug is 'Invalid':
//    - Explain that we will mark this bug as 'Invalid'.
//    Example:
//    For these reasons, and given the use of an unsupported OpenStack version, we are marking this bug as **'Invalid'**.
//    If you still believe this is a Nova bug and you can reproduce it on a supported OpenStack version, please feel free to update this report with the necessary details (referencing our bug reporting template: https://wiki.openstack.org/wiki/Nova/BugsTeam/BugReportTemplate) and set its status back to 'New'.
//
// Revision workflow:
//
// 8- The conversation may include a previously generated message.
// 9- If the next prompt contains the previously generated message with annotations in the form:
//    action -->[text to modify]
//    then you must only modify the indicated part while keeping the rest of the message unchanged.
// 10- The annotation indicates the exact part that must be rewritten or improved.
// 11- If the next prompt does NOT contain the previously generated message, assume the previous answer was not satisfactory and generate a completely new answer following the instructions above and the guidance provided in the latest prompt.
// 12- The final output must always be the full final message (not only the modified fragment).
//
// Here is the bug reported:".to_string()
// }

const RELEASES_URL: &str = "https://releases.openstack.org/";

/// Fetch the OpenStack releases page and extract maintained versions.
/// Returns a formatted list like "- 2025.2 (Flamingo)\n- 2025.1 (Epoxy)"
/// Falls back to a warning message if the fetch fails.
pub(crate) async fn fetch_supported_versions() -> String {
    match reqwest::get(RELEASES_URL).await {
        Ok(response) => match response.text().await {
            Ok(html) => {
                let versions = parse_maintained_versions(&html);
                if versions.is_empty() {
                    warn!("No maintained versions found on {RELEASES_URL}");
                    "Could not determine supported versions. Check https://releases.openstack.org/"
                        .to_string()
                } else {
                    info!("Fetched supported OpenStack versions: {versions}");
                    versions
                }
            }
            Err(e) => {
                warn!("Failed to read releases page: {e}");
                "Could not determine supported versions. Check https://releases.openstack.org/"
                    .to_string()
            }
        },
        Err(e) => {
            warn!("Failed to fetch releases page: {e}");
            "Could not determine supported versions. Check https://releases.openstack.org/"
                .to_string()
        }
    }
}

const ROSTER_URL: &str = "https://etherpad.opendev.org/p/nova-bug-triage-roster/export/txt";

/// Fetch the nova bug triage roster etherpad and extract bug IDs that are already assigned.
/// Returns `Some(set)` with bug IDs found in URLs like `https://bugs.launchpad.net/nova/+bug/NNNNNN`,
/// or `None` if the page could not be fetched.
pub(crate) async fn fetch_roster_bug_ids() -> Option<HashSet<u32>> {
    fetch_roster_bug_ids_from(ROSTER_URL).await
}

async fn fetch_roster_bug_ids_from(url: &str) -> Option<HashSet<u32>> {
    match reqwest::get(url).await {
        Ok(response) => match response.text().await {
            Ok(text) => {
                let ids = parse_roster_bug_ids(&text);
                let mut sorted: Vec<_> = ids.iter().collect();
                sorted.sort();
                info!("Fetched {} bug IDs from triage roster", ids.len());
                debug!("Roster bug IDs: {sorted:?}");
                Some(ids)
            }
            Err(e) => {
                warn!("Failed to read triage roster page: {e}");
                None
            }
        },
        Err(e) => {
            warn!("Failed to fetch triage roster page: {e}");
            None
        }
    }
}

/// Parse plain text from the etherpad and extract bug IDs from Launchpad URLs.
fn parse_roster_bug_ids(text: &str) -> HashSet<u32> {
    let re = Regex::new(r"bugs\.launchpad\.net/nova/\+bug/(\d+)").unwrap();
    re.captures_iter(text)
        .filter_map(|caps| caps[1].parse::<u32>().ok())
        .collect()
}

/// Parse the HTML from releases.openstack.org to extract maintained release names.
/// Looks for table rows where the second cell contains exactly "Maintained".
fn parse_maintained_versions(html: &str) -> String {
    let document = Html::parse_document(html);
    let tr_selector = Selector::parse("tr").unwrap();
    let td_selector = Selector::parse("td").unwrap();

    let versions: Vec<String> = document
        .select(&tr_selector)
        .filter_map(|row| {
            let cells: Vec<_> = row.select(&td_selector).collect();
            if cells.len() >= 2 {
                let status = cells[1].text().collect::<String>();
                if status.contains("Maintained") && !status.contains("Unmaintained") {
                    let name = cells[0].text().collect::<String>();
                    let name = name.trim();
                    if !name.is_empty() {
                        return Some(format!("- {name}"));
                    }
                }
            }
            None
        })
        .collect();

    versions.join("\n")
}

pub(crate) fn get_system_instruction(supported_versions: &str) -> String {
    format!("You are an OpenStack Nova bug triager. Your task is to generate a reply to a bug reporter according to the rules below.
Here is the template for bug submission with all the required information:
*** Start template ***

Description
===========
Some prose which explains more in detail what this bug report is about.

Steps to reproduce
==================
A chronological list of steps which will bring off the issue you noticed:
* I did X
* then I did Y
* then I did Z

Expected result
===============
After the execution of the steps above, what should have happened if the issue wasn't present?

Actual result
=============
What happened instead of the expected result?

Environment
===========
1. Exact version of OpenStack you are running.

   If this is from a distro please provide:
       $ dpkg -l | grep <projectname>
       or
       $ rpm -qa | grep <projectname>

   If this is from git please provide:
       $ git log -1

2. Storage type used (if relevant):
   Examples: Ceph, LVM, GPFS

Logs & Configs
==============
Provide logs like stacktrace or from openstack services using debug mode.

*** End template ***

Bug report template reference:
https://wiki.openstack.org/wiki/Nova/BugsTeam/BugReportTemplate

Currently supported OpenStack versions (maintained releases):
{supported_versions}

Any version NOT in this list is unsupported (end of life).
Supported OpenStack releases page: https://releases.openstack.org/

Instruction to craft the answer:

1. The answer must be plain text.
2. The tone must be professional, concise, and friendly.
3. Thank the reporter for submitting the bug.
4. If the OpenStack version mentioned in the report is not in the supported versions list above, inform the reporter and provide only the link to the supported releases page. Do not list supported versions in the answer.
5. If required information from the bug template is missing, clearly list the missing information and include the link to the bug reporting template for reference.
6. If the bug report is complete and contains all required information, do NOT reference the bug reporting template.

Handling bug status:

If the bug should be marked **Incomplete**:
Explain that the bug will be marked as 'Incomplete', and ask the reporter to set it back to 'New' once the missing information is provided.

Example wording:
For now, we’ll mark this bug as 'Incomplete'. Please update the report with the missing information and set it back to 'New' once updated.

If the bug should be marked **Invalid**:
Explain that the bug will be marked as 'Invalid'.

Example wording:
For these reasons, and given the use of an unsupported OpenStack version, we are marking this bug as **'Invalid'**.

If you still believe this is a Nova bug and you can reproduce it on a supported OpenStack version, please feel free to update this report with the necessary details (referencing our bug reporting template: https://wiki.openstack.org/wiki/Nova/BugsTeam/BugReportTemplate) and set its status back to 'New'.

Triage reasoning (internal step):

Before writing the final answer, internally determine:

- The OpenStack version mentioned in the bug report.
- Whether the version appears to be supported.
- Which template sections are missing or incomplete.
- Whether the bug should likely be marked Incomplete or Invalid.

This reasoning step is internal and must not appear in the final output.

Revision workflow:

The conversation may include a previously generated message.
If the next prompt contains the previously generated message with annotations in the form:

action -->[text to modify]

then you must ONLY modify the indicated part.

Do not modify any other part of the message.
Preserve the wording and formatting of the rest of the message.

If the next prompt does NOT contain the previously generated message, assume the previous answer was not satisfactory and generate a completely new answer following the instructions above and the guidance provided in the latest prompt.

Output requirements:

- The final output must always be the full final message.
- Do not output explanations about the instructions.
- Do not include the internal reasoning.
- Only output the message intended for the bug reporter.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_maintained_versions() {
        let html = r#"<table>
<tr class="row-odd"><td><p><a class="reference internal" href="gazpacho/index.html"><span class="doc">2026.1 Gazpacho</span></a></p></td>
<td><p><a class="reference external" href="https://docs.openstack.org/project-team-guide/stable-branches.html#maintenance-phases">Maintained</a> <em>estimated 2026-04-01</em></p></td>
</tr>
<tr class="row-even"><td><p><a class="reference internal" href="flamingo/index.html"><span class="doc">2025.2 Flamingo</span></a></p></td>
<td><p><a class="reference external" href="https://docs.openstack.org/project-team-guide/stable-branches.html#maintenance-phases">Maintained</a></p></td>
</tr>
<tr class="row-odd"><td><p><a class="reference internal" href="epoxy/index.html"><span class="doc">2025.1 Epoxy</span></a></p></td>
<td><p><a class="reference external" href="https://docs.openstack.org/project-team-guide/stable-branches.html#maintenance-phases">Maintained</a></p></td>
</tr>
<tr class="row-even"><td><p><a class="reference internal" href="dalmatian/index.html"><span class="doc">2024.2 Dalmatian</span></a></p></td>
<td><p><a class="reference external" href="https://docs.openstack.org/project-team-guide/stable-branches.html#maintenance-phases">Maintained</a></p></td>
</tr>
<tr class="row-odd"><td><p><a class="reference internal" href="caracal/index.html"><span class="doc">2024.1 Caracal</span></a></p></td>
<td><p><a class="reference external" href="https://docs.openstack.org/project-team-guide/stable-branches.html#maintenance-phases">Unmaintained</a></p></td>
</tr>
</table>"#;
        let result = parse_maintained_versions(html);
        assert_eq!(
            result,
            "- 2026.1 Gazpacho\n- 2025.2 Flamingo\n- 2025.1 Epoxy\n- 2024.2 Dalmatian"
        );
    }

    #[test]
    fn test_parse_maintained_versions_none_maintained() {
        let html = r##"<table>
<tr class="row-odd"><td><p><span class="doc">2024.1 Caracal</span></p></td>
<td><p><a href="#">Unmaintained</a></p></td>
</tr>
</table>"##;
        assert_eq!(parse_maintained_versions(html), "");
    }

    #[test]
    fn test_parse_maintained_versions_empty_html() {
        assert_eq!(parse_maintained_versions(""), "");
    }

    #[test]
    fn test_parse_roster_bug_ids() {
        let text = r#"
Uggla
https://bugs.launchpad.net/nova/+bug/2062145 - Some bug title
https://bugs.launchpad.net/nova/+bug/2115870 - Another bug

elodilles
https://bugs.launchpad.net/nova/+bug/2098496 - Yet another
Some text without a bug link
"#;
        let ids = parse_roster_bug_ids(text);
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&2062145));
        assert!(ids.contains(&2115870));
        assert!(ids.contains(&2098496));
    }

    #[test]
    fn test_parse_roster_bug_ids_empty() {
        assert!(parse_roster_bug_ids("").is_empty());
    }

    #[test]
    fn test_parse_roster_bug_ids_no_match() {
        assert!(parse_roster_bug_ids("no bugs here, just text").is_empty());
    }

    #[tokio::test]
    async fn test_fetch_roster_bug_ids_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/roster")
            .with_status(200)
            .with_body(
                "Uggla\nhttps://bugs.launchpad.net/nova/+bug/2062145 - title\n\
                 https://bugs.launchpad.net/nova/+bug/2098496 - other\n",
            )
            .create_async()
            .await;

        let url = format!("{}/roster", server.url());
        let result = fetch_roster_bug_ids_from(&url).await;

        mock.assert_async().await;
        let ids = result.expect("should return Some");
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&2062145));
        assert!(ids.contains(&2098496));
    }

    #[tokio::test]
    async fn test_fetch_roster_bug_ids_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/roster")
            .with_status(200)
            .with_body("")
            .create_async()
            .await;

        let url = format!("{}/roster", server.url());
        let result = fetch_roster_bug_ids_from(&url).await;

        mock.assert_async().await;
        let ids = result.expect("should return Some even if empty");
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_roster_bug_ids_connection_refused() {
        // Use a URL that will fail to connect
        let result = fetch_roster_bug_ids_from("http://127.0.0.1:1").await;
        assert!(result.is_none(), "should return None when connection fails");
    }
}
