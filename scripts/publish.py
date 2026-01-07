#!/usr/bin/env python3
import sys
import datetime
import os

def slugify(text):
    return text.lower().replace(" ", "-").replace(":", "").replace("?", "")

def create_post():
    if len(sys.argv) < 2:
        print("Usage: ./scripts/publish.py 'Post Title'")
        sys.exit(1)
        
    title = sys.argv[1]
    today = datetime.date.today()
    slug = f"{today}-{slugify(title)}"
    filename = f"content/blog/{slug}.md"
    
    content = f'''+++
title = "{title}"
date = {today}
description = "Enter description here."
[taxonomies]
tags = ["Unsorted"]
categories = ["Journal"]
+++

Write your content here...
'''
    
    os.makedirs(os.path.dirname(filename), exist_ok=True)
    with open(filename, "w") as f:
        f.write(content)
        
    print(f"✅ Created new post: {filename}")

if __name__ == "__main__":
    create_post()