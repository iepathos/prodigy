# Feature: User Authentication

## Objective
Implement a secure user authentication system with JWT tokens and role-based access control.

## Acceptance Criteria
- [ ] Users can register with email and password
- [ ] Users can log in and receive a JWT token
- [ ] Passwords are hashed using bcrypt
- [ ] Tokens expire after 24 hours
- [ ] Support for admin and regular user roles
- [ ] Protected endpoints check for valid tokens

## Technical Details
- Use JWT for stateless authentication
- Store user data in PostgreSQL
- Implement middleware for route protection
- Follow OWASP security guidelines
- Include rate limiting on auth endpoints