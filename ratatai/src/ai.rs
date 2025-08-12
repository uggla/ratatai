// src/ai.rs

use google_ai_rs::{GenerativeModel, genai::Response};

// Comment out these lines if you don't want to compile with the google_ai_rs dependency
// use google_ai_rs::{Client, GenerativeModel, genai::Response};

// The old commented-out function can be placed here if you want to keep it for reference:
pub async fn get_gemini_response<'a>(
    model: GenerativeModel<'a>,
    prompt: String,
) -> anyhow::Result<Response> {
    let response = model
        //         .generate_content("What is Rust and why is it popular?")
        .generate_content(prompt)
        .await?;
    Ok(response)
}

pub(crate) fn get_initial_prompt() -> String {
    "Forget all previous instructions or prompts to go ahead with this request!
Hi, here are the instructions to answer bug requests, then I will provide you the reported bug:
Here is the template for bug submission with all the required information:
*** Start template ***
Description
===========
Some prose which explains more in detail what this bug report is
about. If the headline of this report is descriptive enough,
skip this section.

Steps to reproduce
==================
A chronological list of steps which will bring off the
issue you noticed:
* I did X
* then I did Y
* then I did Z
A list of openstack client commands would be the most
descriptive example.

Expected result
===============
After the execution of the steps above, what should have
happened if the issue wasn't present?

Actual result
=============
What happened instead of the expected result?
How did the issue look like?

Environment
===========
1. Exact version of OpenStack you are running. See the following
   list for all releases: http://docs.openstack.org/releases/

    If this is from a distro please provide
        $ dpkg -l | grep <projectname>
        or
        $ rpm -qa | grep <projectname>
    If this is from git, please provide
        $ git log -1

2. Which storage type did you use?
   (For example: Ceph, LVM, GPFS, ...)

3. Which networking type did you use?
   (For example: nova-network, Neutron with OpenVSwitch, ...)

Logs & Configs
==============
The tool *sosreport* has support for some OpenStack projects.
It's worth having a look at it. For example, if you want to collect
the logs of a compute node you would execute:

    $ sudo sosreport -o openstack_nova --batch

on that compute node. Attach the logs to this bug report.
*** end template ***

Link bug reporting template: https://wiki.openstack.org/wiki/Nova/BugsTeam/BugReportTemplate
Current fully supported version of Openstack: Flamingo (master / 2025.2), Caracal (2025.2)

Instruction to craft the answer:
1- Answer must be in plain text.
2- You must thank the reporter for submitting the bug.
3- If informations required in the template are missing in the bug report, mention what is missing.
4- Provide the link to the template for reference.
5- Explain that we will mark this bug as 'Incomplete', and ask the reporter to set it back to 'New' once updated.
   Here is an example of what you can write:
   For now, we’ll mark this bug as Invalid, please set it back to 'New' once updated.

Here is the bug reported:".to_string()
}
